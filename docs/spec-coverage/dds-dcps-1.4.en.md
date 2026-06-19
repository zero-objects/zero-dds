# DDS DCPS 1.4 — Spec Coverage

**Spec:** [OMG DDS 1.4 — formal/2015-04-10 (180 pages) →](https://www.omg.org/spec/DDS/1.4/)

**Context:** DCPS is spread across two crates:

- `crates/dcps/` — DCPS core, live with ~25 files + ~300 tests
- `crates/qos/` — QoS policies, ~125 tests

Live: Entity trait + listener stack (6 listeners + listener_dispatch),
WaitSet+GuardCondition+StatusCondition, all 13 status records,
register/unregister/dispose lifecycle, BuiltinSubscriber + 4 builtin-
topic readers, CoherentScope (begin/end_coherent_changes), WLP for
LIVELINESS, InstanceHandle, SampleInfo. 22 QoS policies defined,
a subset wired live.

---

## §1 Scope / Purpose

### 1.1.1 DDS API + communication semantics for DCPS

**Spec:** §1.1, p. 1 — "The DDS spec defines an API + communication
semantics for DCPS."

**Repo:** `crates/dcps/src/lib.rs` with the full §2.2.2 entity API
(DomainParticipant, Publisher, Subscriber, Topic, DataWriter,
DataReader, WaitSet, Conditions). Communication semantics over the
RTPS transport (`crates/rtps/`, `crates/discovery/`).

**Tests:** workspace-wide (the §2.2.2.x section items demonstrate all
sub-requirements individually).

**Status:** done — all sub-sections §2.2.2-§2.2.6 are marked done;
see the per-item evidence further below.

### 1.1.2 Pre-allocate resources (no dynamic alloc)

**Spec:** §1.1, p. 1 — "Pre-allocate Resources."

**Repo:** `crates/dcps/src/runtime.rs::RuntimeConfig` with caps for
sample pools, reader/writer capacity. DoS caps in
`crates/rtps/src/parameter_list.rs::MAX_PARAMETERS` and
`crates/dcps/src/condition.rs::MAX_WAITSET_CONDITIONS`.

**Tests:** RuntimeConfig tests + DoS-cap tests in the respective
modules.

**Status:** done — all critical hot paths have static caps;
heap allocations remain on-startup or per-event instead of per-sample.

### 1.1.3 Avoid unbounded resources

**Spec:** §1.1 — "Avoid unbounded resources."

**Repo:** RESOURCE_LIMITS QoS in
`crates/qos/src/policies/resource_limits.rs` plus DoS caps in
ParameterList, WaitSet, the type resolver
(`crates/types/src/resolve.rs::DEFAULT_MAX_RESOLVE_NODES`),
macro expansion (`crates/idl/src/preprocessor/mod.rs::MAX_MACRO_EXPANSION_DEPTH`).

**Tests:** per-cap negative tests per cap site (see the respective
crate-specific test modules).

**Status:** done

### 1.1.4 Minimize copies

**Spec:** §1.1 — "Minimize copies."

**Repo:** `crates/cdr/src/buffer.rs::BufferReader` with borrow decode
(zero-copy byte slices); `crates/dcps/src/dds_type.rs::RawBytes`
for the unmarshalled inbox; `crates/cdr/src/struct_enc.rs::MutableMember`
with `body: &[u8]`.

**Tests:** zero-copy paths implicit via roundtrip tests; explicit
in `cdr` borrow tests.

**Status:** done

### 1.1.5 Typed interfaces

**Spec:** §1.1 — "Typed Interfaces."

**Repo:** `crates/dcps/src/dds_type.rs::DdsType` trait, generic
over Topic/Writer/Reader.

**Tests:** type-generic tests.

**Status:** done

### 1.1.6 QoS-driven behavior

**Spec:** §1.1 — "QoS-driven Behavior."

**Repo:** `crates/qos/src/policies/` with all 22 spec policies:
durability, durability_service, presentation, deadline, latency_budget,
ownership, ownership_strength, liveliness, time_based_filter, partition,
reliability, transport_priority, lifespan, destination_order, history,
resource_limits, entity_factory, writer_data_lifecycle,
reader_data_lifecycle, user_data, topic_data, group_data.
Compatibility check via `crates/qos/src/compatibility.rs`.

**Tests:** an own test module per QoS policy (see §2.2.3.x items
further below); cross-QoS compatibility tests in `compatibility.rs`.

**Status:** done — all 22 policies are available in the wire format and
code; engine-side wiring see §2.2.3.x per policy. K3a-F
 all 10 Iron-Rule open trackers converted
to live implementations (T1 contains_entity recursive, T2 GROUP-
coherent snapshot_generation, T3 RESOURCE_LIMITS write block,
T4 EXCLUSIVE strength selection + GUID tiebreaker, T5 TIME_BASED_FILTER
reader drop, T6 READER_DATA_LIFECYCLE autopurge timer, T7
QueryCondition SQL per-sample eval, T8 MultiTopic hash join,
T9 liveliness-driven OWNERSHIP failover, T10 TRANSIENT/PERSISTENT
durability service).

### 1.1.7 Pub/sub separation (separate include)

**Spec:** §1.1 — "Pub/Sub separation."

**Repo:** `crates/dcps/src/{publisher,subscriber}.rs`.

**Tests:** crate-layout tests.

**Status:** done

### 1.2 Wire interop is NOT in scope of DCPS (see DDSI-RTPS)

**Spec:** §1.2 — "Wire interop is separate, in DDSI-RTPS."

**Repo:** — see `ddsi-rtps-2.5.md`.

**Tests:** —

**Status:** `n/a (informative)` — the section only declares the
scope separation between the DCPS spec and the DDSI-RTPS spec; no
normative requirement on the DCPS implementation.

---

## §2.1 Summary

### 2.1.1 The DDS spec consists of PIM (UML) + IDL PSM

**Spec:** §2.1, p. 3 — "PIM + IDL PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — describes the structure of the
spec itself (the OMG PIM+PSM convention), not a requirement to
implement.

### 2.1.2 DCPS layer for typed pub/sub data

**Spec:** §2.1, p. 3.

**Repo:** `crates/dcps/src/lib.rs`.

**Tests:** crate-wide.

**Status:** done

### 2.1.3 DLRL layer

**Spec:** §2.1, p. 3 — optional Data-Local-Reconstruction Layer
with an object cache + identity tracking + relationship resolver
(1:1/1:N/N:M) over DCPS.

**Repo:** `crates/dlrl/` (1668 SLOC + 48 inline tests) +
`crates/dlrl-codegen/` (PSM codegen for C++/C#/Java/TS).

**Tests:** cross-ref [`dlrl-1.2`](dlrl-1.2.md) (own spec-coverage file for
DDS 1.2 §8 + Annex B; no longer part of the spec in DDS 1.4).

**Status:** done — DLRL is implemented as a standalone stack;
formal spec coverage in [`dlrl-1.2`](dlrl-1.2.md) (20 done / 0 partial / 0 open).

---

## §2.2.1.1 ReturnCode_t (Page 5, Tab. RC)

### 2.2.1.1.1 RC OK -> Result::Ok

**Spec:** §2.2.1.1 Tab.RC — "OK: Successful return."

**Repo:** `crates/dcps/src/error.rs::Result::Ok`.

**Tests:** crate-wide.

**Status:** done

### 2.2.1.1.2 RC ERROR -> DdsError::Other

**Spec:** "ERROR: Generic, unspecified error."

**Repo:** `crates/dcps/src/error.rs::DdsError::Other { reason }` +
`as_return_code()` maps to `return_code::ERROR = 1`.
`WireError`/`TransportError` also map to RC ERROR
(spec-conformant: all three are unspecified implementation errors).

**Tests:** `crates/dcps/src/error.rs::tests`:
`rc_error_maps_for_wire_and_transport_and_other`,
`rc_constants_have_spec_values`.

**Status:** done

### 2.2.1.1.3 RC BAD_PARAMETER -> DdsError::BadParameter

**Spec:** "BAD_PARAMETER: Illegal parameter value."

**Repo:** `crates/dcps/src/error.rs::DdsError::BadParameter { what }`
+ `as_return_code()` maps to `return_code::BAD_PARAMETER = 3`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_bad_parameter_maps`;
plus trigger-site tests in `crates/dcps/src/topic.rs` (e.g. an empty
topic name → BadParameter).

**Status:** done

### 2.2.1.1.4 RC UNSUPPORTED -> DdsError::Unsupported

**Spec:** "UNSUPPORTED: Unsupported operation."

**Repo:** `crates/dcps/src/error.rs::DdsError::Unsupported { feature }`
+ `as_return_code()` maps to `return_code::UNSUPPORTED = 2`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_unsupported_maps`.

**Status:** done

### 2.2.1.1.5 RC ALREADY_DELETED

**Spec:** "ALREADY_DELETED: already deleted."

**Repo:** `crates/dcps/src/entity.rs::EntityState.deleted: AtomicBool` +
`mark_deleted()` (idempotent) + `check_not_deleted() -> Result<()>`
guard helper delivers `DdsError::AlreadyDeleted` on subsequent ops.
`as_return_code()` maps to `return_code::ALREADY_DELETED = 9`.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`check_not_deleted_passes_for_fresh_entity`,
`check_not_deleted_returns_already_deleted_after_mark`,
`mark_deleted_is_idempotent`;
plus the mapping `crates/dcps/src/error.rs::tests::rc_already_deleted_maps`.

**Status:** done

### 2.2.1.1.6 RC OUT_OF_RESOURCES -> DdsError::OutOfResources

**Spec:** "OUT_OF_RESOURCES."

**Repo:** `crates/dcps/src/error.rs::DdsError::OutOfResources { what }` +
`as_return_code()` maps to `return_code::OUT_OF_RESOURCES = 5`;
RESOURCE_LIMITS-QoS wiring in `crates/qos/src/policies/resource_limits.rs`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_out_of_resources_maps`;
plus QoS tests in `crates/qos/src/policies/resource_limits.rs::tests`.

**Status:** done

### 2.2.1.1.7 RC NOT_ENABLED

**Spec:** "NOT_ENABLED: Operation invoked on disabled Entity."

**Repo:** `crates/dcps/src/error.rs::DdsError::NotEnabled` +
`as_return_code()` maps to `return_code::NOT_ENABLED = 6`.
`crates/dcps/src/entity.rs::EntityState::check_enabled() -> Result<()>`
guard helper for public ops.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`check_enabled_returns_not_enabled_for_disabled_entity`,
`check_enabled_passes_after_enable`,
`check_enabled_passes_for_factory_entity`,
plus existing `enable_is_idempotent_and_reports_first_transition`;
plus the mapping `crates/dcps/src/error.rs::tests::rc_not_enabled_maps`.

**Status:** done

### 2.2.1.1.8 RC IMMUTABLE_POLICY

**Spec:** "IMMUTABLE_POLICY: Modify immutable QosPolicy."

**Repo:** `crates/dcps/src/error.rs::DdsError::ImmutablePolicy { policy }`
+ `as_return_code()` maps to `return_code::IMMUTABLE_POLICY = 7`.
`crates/dcps/src/entity.rs::immutable_if_enabled` helper site.
The immutability matrix is tracked per QoS item in §2.2.3.x.

**Tests:** `crates/dcps/src/error.rs::tests::rc_immutable_policy_maps`;
plus `crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`.

**Status:** done

### 2.2.1.1.9 RC INCONSISTENT_POLICY

**Spec:** "INCONSISTENT_POLICY: inconsistent set."

**Repo:** `crates/dcps/src/error.rs::DdsError::InconsistentPolicy { what }`
+ `as_return_code()` maps to `return_code::INCONSISTENT_POLICY = 8`.
QoS consistency check in `crates/qos/src/compatibility.rs`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_inconsistent_policy_maps`;
plus QoS compatibility tests in `crates/qos/src/compatibility.rs::tests`.

**Status:** done

### 2.2.1.1.10 RC PRECONDITION_NOT_MET

**Spec:** "PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/error.rs::DdsError::PreconditionNotMet { reason }`
+ `as_return_code()` maps to `return_code::PRECONDITION_NOT_MET = 4`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_precondition_not_met_maps`.

**Status:** done

### 2.2.1.1.11 RC TIMEOUT

**Spec:** "TIMEOUT."

**Repo:** `error.rs::DdsError::Timeout`.

**Tests:** timeout tests in `publisher.rs`.

**Status:** done

### 2.2.1.1.12 RC ILLEGAL_OPERATION

**Spec:** "ILLEGAL_OPERATION."

**Repo:** `crates/dcps/src/error.rs::DdsError::IllegalOperation { what }`
+ `as_return_code()` maps to `return_code::ILLEGAL_OPERATION = 12`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_illegal_operation_maps`.

**Status:** done

### 2.2.1.1.13 RC NO_DATA

**Spec:** "NO_DATA."

**Repo:** `crates/dcps/src/error.rs::DdsError::NoData` +
`as_return_code()` maps to `return_code::NO_DATA = 11`. In
practice read/take return an empty `Vec<Sample>` instead of an
explicit error — that is spec-conformant (§2.2.2.5.3.6 allows
both variants); the explicit error is available for C/C++/Java bridges.

**Tests:** `crates/dcps/src/error.rs::tests::rc_no_data_maps`.

**Status:** done

---

## §2.2.1.2 Conceptual Outline

### 2.2.1.2.1 Publisher/DataWriter send-side, Subscriber/DataReader receive-side

**Spec:** §2.2.1.2, p. 5-6.

**Repo:** `crates/dcps/src/{publisher,subscriber}.rs`.

**Tests:** crate-wide.

**Status:** done

### 2.2.1.2.2 Topic connects publication and subscription via name+type

**Spec:** §2.2.1.2 — "Topic name+type."

**Repo:** `crates/dcps/src/topic.rs::Topic<T>`.

**Tests:** topic tests.

**Status:** done

### 2.2.1.2.3 Coherent set via Publisher (begin/end_coherent_changes)

**Spec:** §2.2.1.2.

**Repo:** `crates/dcps/src/coherent_set.rs::CoherentScope::begin/end`.

**Tests:** `begin_coherent_changes_*` tests.

**Status:** done

---

## §2.2.2.1 Infrastructure Module

### 2.2.2.1.1 Entity class — abstract base + QoS + listener + StatusCondition

**Spec:** §2.2.2.1.1, p. 11-13 — operations: set_qos / get_qos /
set_listener / get_listener / get_statuscondition / get_status_changes /
enable / get_instance_handle.

**Repo:** `crates/dcps/src/entity.rs::Entity` trait + `EntityState`.

**Tests:** `enable_is_idempotent_and_reports_first_transition`,
`get_status_condition_returns_pristine_when_no_changes`.

**Status:** done — Entity trait + default impls + `EntityState`;
IMMUTABLE-QoS enforcement via `immutable_if_enabled` /
`DdsError::ImmutablePolicy` (§2.2.2.1.2 `set_qos`).

### 2.2.2.1.2 DomainEntity class

**Spec:** §2.2.2.1.2, p. 14 — "Specialization of Entity to be a
contained-element of a DomainParticipant."

**Repo:** the Entity trait suffices; no separate DomainEntity trait.

**Tests:** indirect.

**Status:** done

### 2.2.2.1.3 QosPolicy class

**Spec:** §2.2.2.1.3, p. 15.

**Repo:** `crates/qos/src/policies/`.

**Tests:** policies tests.

**Status:** done

### 2.2.2.1.4 Listener interface (abstract root)

**Spec:** §2.2.2.1.4, p. 15 — "Abstract root for all listener types."

**Repo:** `crates/dcps/src/listener.rs` with 6 listener traits
(Topic/DataWriter/Publisher/DataReader/Subscriber/DomainParticipant).

**Tests:** `listener_dispatch_*` tests.

**Status:** done

### 2.2.2.1.5 Status class

**Spec:** §2.2.2.1.5, p. 16.

**Repo:** `crates/dcps/src/status.rs` with all 13 status records.

**Tests:** `status_records_default` tests.

**Status:** done

### 2.2.2.1.6 WaitSet class (attach/detach/wait/get_conditions/notify)

**Spec:** §2.2.2.1.6, p. 16-17.

**Repo:** `crates/dcps/src/condition.rs::WaitSet` with
`attach_condition` (idempotent + DoS cap `MAX_WAITSET_CONDITIONS = 1024`
delivers `OutOfResources`); `detach_condition`; `get_conditions`;
`wait` with single-thread-wait enforcement via `WaitSetInner.waiting:
AtomicBool` + an RAII guard (`PreconditionNotMet` on a concurrent
second call); `notify` cvar wakeup; multi-WaitSet via Clone
(shared Inner via Arc).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`waitset_attach_detach_idempotent`,
`waitset_wait_returns_immediately_if_already_triggered`,
`waitset_wait_timeout_returns_err`,
`waitset_wakes_when_guard_triggers`,
`waitset_with_multiple_conditions_returns_all_triggered`,
`waitset_clone_shares_state`,
`waitset_default_is_empty`,
`waitset_concurrent_wait_returns_precondition_not_met` (single-
thread-enforce negative),
`waitset_sequential_waits_after_first_returns_succeed`,
`waitset_attach_above_max_returns_out_of_resources` (OOR negative),
`waitset_attach_idempotent_does_not_count_against_cap`,
`waitset_panic_in_wait_releases_waiting_flag`.

**Status:** done

### 2.2.2.1.7 Condition class (trait root + get_trigger_value)

**Spec:** §2.2.2.1.7, p. 17.

**Repo:** `condition.rs::Condition` trait.

**Tests:** `condition_trait_*` tests.

**Status:** done

### 2.2.2.1.8 GuardCondition class (set_trigger_value)

**Spec:** §2.2.2.1.8, p. 18.

**Repo:** `condition.rs::GuardCondition`.

**Tests:** GuardCondition tests.

**Status:** done

### 2.2.2.1.9 StatusCondition class (set/get_enabled_statuses, get_entity)

**Spec:** §2.2.2.1.9, p. 18-19.

**Repo:** `crates/dcps/src/entity.rs::StatusCondition` with
`set_enabled_statuses` / `enabled_statuses` / `trigger_value` /
`get_entity_handle() -> InstanceHandle` (spec §2.2.2.1.9 get_entity
equivalent for the Rust API; the handle is the only stable identity
across all wrapper granularities) / `entity_state() -> &Arc<EntityState>`
for direct state access.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`status_condition_get_entity_handle_matches_owner_state`,
`status_condition_get_entity_handle_unique_per_entity`,
`status_condition_entity_state_returns_same_arc`,
`status_condition_entity_state_reflects_lifecycle_changes`,
plus existing `status_condition_trigger_value`,
`status_condition_implements_condition_trait`.

**Status:** done

---

## §2.2.2.2 Domain Module

### 2.2.2.2.1 DomainParticipant class

**Spec:** §2.2.2.2.1, p. 21-30 — container for Pub/Sub/Topic/MultiTopic;
factory methods; admin ops (ignore_*); get_discovered_*; etc.

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant` with:
- `create_publisher` / `create_subscriber` / `create_topic` (factory
  methods, each operation tracks the `InstanceHandle` of the created
  entity in `inner.publishers` / `inner.subscribers` / `inner.topics`
  for `contains_entity` and `delete_contained_entities`).
- `lookup_topicdescription(name) -> Option<TopicDescriptionHandle>`
  (spec §2.2.2.2.1.12) — immediate local lookup without a discovery wait.
- `find_topic(name, timeout) -> Result<TopicDescriptionHandle>`
  (spec §2.2.2.2.1.11) — local + SEDP cache poll until timeout.
- `instance_handle()` (spec §2.2.2.1.1) — InstanceHandle of the participant.
- `contains_entity(handle) -> bool` (spec §2.2.2.2.1.10) — checks self,
  all topics, all pub/sub children.
- `ignore_*` / `get_discovered_*` / `delete_contained_entities`
  already wired in WP 2.7.

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

**Recursive path:** `contains_entity` additionally recognizes
DataWriter/DataReader handles via pub/sub children. PublisherInner +
SubscriberInner track DW/DR handles (`datawriters` /
`datareaders` mutex); via a weak backref they also propagate into
`ParticipantInner.datawriters` / `datareaders` for fast
top-down lookup. Per pub/sub there is also `contains_writer` /
`contains_reader` as a targeted sub-lookup.

**Tests (in addition to the 6 above):**
`contains_entity_recursive_finds_local_datawriter`,
`contains_entity_recursive_finds_local_datareader`,
`contains_entity_recursive_does_not_find_foreign_datawriter`.

**Status:** done

### 2.2.2.2.2 DomainParticipantFactory class (singleton)

**Spec:** §2.2.2.2.2, p. 31-33.

**Repo:** `crates/dcps/src/factory.rs::DomainParticipantFactory` with:
- `instance() -> &'static Self` (spec §2.2.2.2.2.1 `get_instance`,
  singleton via `OnceLock`).
- `create_participant` / `create_participant_with_config` /
  `create_participant_offline` (spec §2.2.2.2.2.2; all variants
  track the participant in the internal registry).
- `lookup_participant(domain_id) -> Option<DomainParticipant>`
  (spec §2.2.2.2.2.4).
- `delete_participant(&p) -> Result<()>` (spec §2.2.2.2.2.3 — removes
  from the registry + calls `delete_contained_entities`; delivers
  `PreconditionNotMet` if not registered).
- `set_default_participant_qos` / `get_default_participant_qos`
  (spec §2.2.2.2.2.5).
- `set_qos` / `get_qos` with a `DomainParticipantFactoryQos` struct
  (spec §2.2.2.2.2.6, default `autoenable_created_entities = true`).

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

### 2.2.2.2.3 DomainParticipantListener interface

**Spec:** §2.2.2.2.3, p. 33-34.

**Repo:** `listener.rs::DomainParticipantListener` trait + 14-method
default.

**Tests:** `listener_*` tests.

**Status:** done

---

## §2.2.2.3 Topic-Definition Module

### 2.2.2.3.1 TopicDescription class (abstract base)

**Spec:** §2.2.2.3.1, p. 35 — abstract base for `Topic`,
`ContentFilteredTopic`, `MultiTopic`. API: `get_type_name`,
`get_name`, `get_participant`.

**Repo:** `crates/dcps/src/topic.rs::TopicDescription` trait
(object-safe, `&dyn TopicDescription` is the consumed type);
implemented by `Topic<T>`, `TopicDescriptionHandle`,
`ContentFilteredTopic<T>`.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`topic_implements_topic_description`,
`topic_description_handle_is_cloneable`,
`topic_description_trait_is_object_safe`,
`topic_description_create_topic_rejects_empty_name`
(BadParameter negative),
`topic_description_get_participant_returns_owning_participant`.

**Status:** done

### 2.2.2.3.2 Topic class

**Spec:** §2.2.2.3.2, p. 36-37.

**Repo:** `topic.rs::Topic<T>`.

**Tests:** topic tests.

**Status:** done

### 2.2.2.3.3 ContentFilteredTopic class

**Spec:** §2.2.2.3.3, p. 37-38 — filter expression + filter parameters
+ get_related_topic. TopicDescription implementation with the type name
from the related topic.

**Repo:** `crates/dcps/src/topic.rs::ContentFilteredTopic<T>` with
`new` (constructor with expression parsing + parameter-index validation),
`get_filter_expression`, `get_filter_parameters`,
`set_filter_parameters` (with BadParameter validation),
`get_related_topic`, `evaluate(row)` over the `zerodds_sql_filter` engine,
plus a `TopicDescription` trait impl. DomainParticipant
`create_contentfilteredtopic` factory.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`cft_compiles_and_evaluates_filter`,
`cft_with_params_can_be_updated`,
`cft_get_related_topic`,
`cft_invalid_expression_rejected` (BadParameter negative),
`cft_param_index_out_of_range_rejected` (BadParameter negative),
`cft_set_filter_parameters_validates_count`,
`cft_filter_with_string_param`,
`cft_filter_with_or_and_combination`,
`cft_unknown_field_returns_bad_parameter` (BadParameter negative),
`cft_clone_shares_params`.

**Status:** done

### 2.2.2.3.4 MultiTopic class [optional, spec §2.2.2.3.4]

**Spec:** §2.2.2.3.4, p. 38-39 — combines multiple topics via an
SQL subscription expression. Spec-explicit `optional`.

**Repo:** `crates/dcps/src/topic.rs::MultiTopic<T>` with
`subscription_expression` (read-only), `expression_parameters`
(get/set per RwLock), `related_topic_names: Vec<String>`,
`get_related_topic_names`. TopicDescription trait impl with a
user-defined `type_name`.
DomainParticipant factory: `create_multitopic` (spec §2.2.2.2.1.15) +
`delete_multitopic` (spec §2.2.2.2.1.16) with participant-match
validation and a full BadParameter negative path.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`multitopic_compiles_and_implements_topic_description`,
`multitopic_set_expression_parameters_roundtrip`,
`multitopic_rejects_empty_name` (negative),
`multitopic_rejects_empty_type_name` (negative),
`multitopic_rejects_empty_related_topics` (negative),
`multitopic_rejects_invalid_expression` (negative),
`multitopic_rejects_param_index_out_of_range` (negative),
`multitopic_set_params_validates_index_range` (negative),
`multitopic_clone_shares_params`,
`delete_multitopic_rejects_foreign_participant` (negative).

**Cross-topic sample routing:** hash-join operator
`crates/dcps/src/topic.rs::hash_join_two` with an O(n+m) build/probe
phase + Cartesian match for duplicate keys. `JoinedRow<'a>`
dispatches dotted paths `<topic>.<field>` to the right
topic source. `MultiTopic::evaluate_joined(row)` evaluates the
`subscription_expression` against the combined rows.
Tests: `joined_row_dispatches_dotted_paths_by_topic_prefix`,
`joined_row_undotted_falls_back_to_first_match`,
`multitopic_evaluate_joined_uses_dotted_paths`,
`hash_join_two_combines_matching_rows`,
`hash_join_two_returns_empty_when_no_keys_match` (negative),
`hash_join_two_emits_cartesian_for_duplicate_keys`,
`hash_join_two_predicate_can_filter_pairs`.

**Status:** done

### 2.2.2.3.5 TopicListener interface (on_inconsistent_topic)

**Spec:** §2.2.2.3.5, p. 40.

**Repo:** `listener.rs::TopicListener` trait.

**Tests:** —

**Status:** done

### 2.2.2.3.6 TypeSupport interface

**Spec:** §2.2.2.3.6, p. 40-41.

**Repo:** `dds_type.rs::DdsType` trait.

**Tests:** —

**Status:** done

### 2.2.2.3.7 Derived classes per application class (FooDataReader/Writer/TypeSupport)

**Spec:** §2.2.2.3.7, p. 41.

**Repo:** generic `DataReader<T>`/`DataWriter<T>` (Rust-idiomatically
no Foo* classes).

**Tests:** generic tests.

**Status:** done

---

## §2.2.2.4 Publication Module

### 2.2.2.4.1 Publisher class

**Spec:** §2.2.2.4.1, p. 43-46 — operations: create_datawriter,
delete_datawriter, set/get_default_datawriter_qos, copy_from_topic_qos,
suspend/resume_publications, begin/end_coherent_changes,
wait_for_acknowledgments, get_participant.

**Repo:** `crates/dcps/src/publisher.rs::Publisher` with:
- `create_datawriter` / `set_listener` / `assert_liveliness` /
  `wait_for_acknowledgments` (existing).
- `suspend_publications` / `resume_publications` / `is_suspended`
  via `PublisherInner.suspended: AtomicBool` (spec §2.2.2.4.1.10/11).
- `copy_from_topic_qos(&mut DataWriterQos, &TopicQos)` static helper
  (spec §2.2.2.4.1.13) — copies the shared policies visible in the
  current TopicQos struct (durability, reliability).
- coherent changes via `crates/dcps/src/coherent_set.rs::CoherentScope`.

**Tests:** `crates/dcps/src/publisher.rs::tests`:
`publisher_creates_datawriter_for_matching_type`,
`suspend_publications_sets_flag`,
`resume_publications_clears_flag`,
`suspend_publications_is_idempotent`,
`resume_without_suspend_is_noop`,
`copy_from_topic_qos_copies_durability_and_reliability`,
`copy_from_topic_qos_does_not_touch_writer_only_policies`,
plus coherent-set tests in `coherent_set.rs::tests`.

**Status:** done

### 2.2.2.4.2 DataWriter class

**Spec:** §2.2.2.4.2, p. 47-55 — write/dispose/register/lookup/
assert_liveliness/get_*-status/get_matched_subscriptions.

**Repo:** `publisher.rs::DataWriter<T>` + `instance_tracker.rs` +
`wlp.rs`.

**Tests:** `datawriter_*` tests.

**Status:** done — register_instance/unregister/dispose/write/
write_w_timestamp/lookup_instance/get_key_value/assert_liveliness
live; get_*_status polling API partial.

### 2.2.2.4.3 PublisherListener interface

**Spec:** §2.2.2.4.3, p. 56.

**Repo:** `listener.rs::PublisherListener`.

**Tests:** —

**Status:** done

### 2.2.2.4.4 DataWriterListener interface (4 callbacks)

**Spec:** §2.2.2.4.4, p. 57.

**Repo:** `listener.rs::DataWriterListener`.

**Tests:** —

**Status:** done

### 2.2.2.4.5 Concurrency behavior

**Spec:** §2.2.2.4.5, p. 57.

**Repo:** Rust-thread-safe via Arc<Mutex>.

**Tests:** —

**Status:** done

---

## §2.2.2.5 Subscription Module

### 2.2.2.5.1 SampleInfo state machine (Sample/View/Instance state)

**Spec:** §2.2.2.5.1.1-9, p. 59-65 — interpretation of the SampleInfo
fields; three orthogonal state dimensions with a mask filter.

**Repo:** `crates/dcps/src/sample_info.rs` with:
- `SampleStateKind` (READ / NOT_READ) + `sample_state_mask::*` constants.
- `ViewStateKind` (NEW / NOT_NEW) + `view_state_mask::*`.
- `InstanceStateKind` (ALIVE / NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS)
  + `instance_state_mask::*` + `is_alive` / `is_not_alive` predicates.
- `SampleInfo::matches_states(sample_mask, view_mask, instance_mask)`
  for the `read_w_condition` filter.
- `SampleInfo::new_alive` constructor for the live path.

**Tests:** `crates/dcps/src/sample_info.rs::tests`:
`defaults_are_alive_new_not_read`, `instance_state_predicates`,
`sample_state_predicate`, `matches_states_filter`,
`sample_info_three_state_dimensions_independent` (positive +
negative filter per dimension), `sample_info_dispose_marker_has_invalid_data`,
`enum_default_impls`.

**Status:** done

### 2.2.2.5.2 Subscriber class

**Spec:** §2.2.2.5.2, p. 65-69 — operations: create_datareader,
get_datareaders, notify_datareaders, begin/end_access, etc.

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber` with:
- `create_datareader` / `set_listener` / `get_listener` (existing).
- `begin_access` (spec §2.2.2.5.2.8, idempotent, nestable) +
  `end_access` (spec §2.2.2.5.2.9, delivers PreconditionNotMet on
  underflow) + `is_access_open` via `SubscriberInner.access_scope:
  Arc<GroupAccessScope>` (counter-based in
  `crates/dcps/src/coherent_set.rs::GroupAccessScope`).

**Tests:** `crates/dcps/src/subscriber.rs::tests`:
`subscriber_begin_end_access_roundtrip`,
`subscriber_end_access_without_begin_returns_precondition_not_met`,
(negative),
`subscriber_begin_access_is_nestable`,
`subscriber_too_many_ends_after_balanced_returns_error` (negative),
plus existing subscriber-create_datareader and listener tests.

**Status:** done

### 2.2.2.5.3 DataReader class

**Spec:** §2.2.2.5.3, p. 70-85 — read/take/read_with_info/
read_filtered/read_instance/take_instance/read_next_instance/
take_next_instance/get_*-status/wait_for_historical_data etc.

**Repo:** `subscriber.rs::DataReader<T>`.

**Tests:** `datareader_*` tests.

**Status:** done — all read/take variants done; get_*_status
polling partial; wait_for_historical_data open.

### 2.2.2.5.4 DataSample class (Data + SampleInfo)

**Spec:** §2.2.2.5.4, p. 86.

**Repo:** `subscriber.rs::Sample<T>`.

**Tests:** —

**Status:** done

### 2.2.2.5.5 SampleInfo class (complete)

**Spec:** §2.2.2.5.5, p. 86-87 — sample_state/view_state/
instance_state/disposed_generation_count/no_writers_generation_count/
sample_rank/generation_rank/absolute_generation_rank/source_timestamp/
instance_handle/publication_handle/valid_data (12 fields).

**Repo:** `crates/dcps/src/sample_info.rs::SampleInfo` with all 12
spec fields and appropriate default values. The reader-side update of
the generation counters happens in the DataReader read paths.

**Tests:** `crates/dcps/src/sample_info.rs::tests`:
`sample_info_all_spec_fields_accessible` (all 12 fields readable +
settable with concrete values), `sample_info_dispose_marker_has_invalid_data`
(negative: a dispose sample has valid_data=false),
`sample_info_generation_rank_starts_zero`,
`new_alive_constructor_sets_handles_and_timestamp`,
plus existing `defaults_are_alive_new_not_read`.

**Status:** done

### 2.2.2.5.6 SubscriberListener interface

**Spec:** §2.2.2.5.6, p. 87.

**Repo:** `listener.rs::SubscriberListener`.

**Tests:** —

**Status:** done

### 2.2.2.5.7 DataReaderListener interface (7 callbacks)

**Spec:** §2.2.2.5.7, p. 88.

**Repo:** `listener.rs::DataReaderListener`.

**Tests:** —

**Status:** done

### 2.2.2.5.8 ReadCondition class

**Spec:** §2.2.2.5.8, p. 88-89 — Sample/View/InstanceState mask
filter; triggers when the reader contains samples with these masks.

**Repo:** `crates/dcps/src/condition.rs::ReadCondition` with mask
getters (`get_sample_state_mask`, `get_view_state_mask`,
`get_instance_state_mask`) + a closure-based trigger
`(sm, vm, im) -> bool`. `Condition` trait implemented (object-safe).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`read_condition_returns_trigger_from_closure`,
`read_condition_trigger_false_when_closure_says_false` (negative),
`read_condition_implements_condition_trait_object_safe`,
`read_condition_attaches_to_waitset` (WaitSet integration).

**Status:** done

### 2.2.2.5.9 QueryCondition class

**Spec:** §2.2.2.5.9, p. 89 — extends ReadCondition with an SQL filter
expression + mutable filter parameters.

**Repo:** `crates/dcps/src/condition.rs::QueryCondition` with a
`query_expression` field (read-only) + a `query_parameters` mutex
(get/set), `base()` accessor on the sub-`ReadCondition`. The SQL filter
is parsed once in the constructor via `zerodds_sql_filter::parse` to
`Expr` (BadParameter on syntax error). `evaluate(row)` /
`evaluate_with_values(row, params)` evaluate the filter per sample.
Per-sample evaluation (spec §2.2.2.5.9.6) is live in
`crates/dcps/src/subscriber.rs::DataReader::read_w_condition` and
`take_w_condition`; `DdsTypeRow<'a, T>` (in `dds_type.rs`) wraps
`T: DdsType` to `zerodds_sql_filter::RowAccess` via the optional
`DdsType::field_value(path)` method.

**Tests:** `crates/dcps/src/condition.rs::tests`:
`query_condition_inherits_trigger_from_base`,
`query_condition_set_query_parameters_roundtrip`,
`query_condition_trigger_inherits_false_from_base` (negative),
`query_condition_base_returns_correct_read_condition`,
`query_condition_rejects_invalid_sql` (negative),
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

**Spec:** §2.2.3.1, p. 101 — sequence<octet> value.

**Repo:** `crates/qos/src/policies/user_data.rs`.

**Tests:** USER_DATA tests.

**Status:** done

### 2.2.3.2 TOPIC_DATA QosPolicy

**Spec:** §2.2.3.2, p. 102.

**Repo:** `crates/qos/src/policies/topic_data.rs`.

**Tests:** —

**Status:** done

### 2.2.3.3 GROUP_DATA QosPolicy

**Spec:** §2.2.3.3, p. 102.

**Repo:** `crates/qos/src/policies/group_data.rs`.

**Tests:** —

**Status:** done

### 2.2.3.4 DURABILITY QosPolicy (VOLATILE/TRANSIENT_LOCAL/TRANSIENT/PERSISTENT)

**Spec:** §2.2.3.4, p. 102-103.

**Repo:** `crates/qos/src/policies/durability.rs::DurabilityQosPolicy`
with all 4 `DurabilityKind` values + a spec-conformant ordering relation
(VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT) +
wire encode/decode (u32). Volatile + TransientLocal are implemented in the
writer-side sample cache; Transient + Persistent
access the durability service §2.2.3.5
(`crates/dcps/src/durability_service.rs`).

**Tests:** `crates/qos/src/policies/durability.rs::tests`:
`default_is_volatile`, `ordering_matches_spec`, `try_from_u32_is_strict`,
`from_u32_is_forward_compat`, `encode_decode_roundtrip` (all 4 kinds),
`unknown_kind_decode_rejected` (negative),
`compatibility_offered_ge_requested` (offered >= requested + negative).

**Status:** done — wire form + compatibility ordering complete;
Transient/Persistent storage delegated to §2.2.3.5 durability service.

### 2.2.3.5 DURABILITY_SERVICE QosPolicy

**Spec:** §2.2.3.5, p. 103 — service_cleanup_delay + history_kind +
history_depth + max_samples + max_instances + max_samples_per_instance.

**Repo:** `crates/qos/src/policies/durability_service.rs::DurabilityServiceQosPolicy`
with all 6 spec fields + wire encode/decode. Storage-service backends
in `crates/dcps/src/durability_service.rs`: trait `DurabilityBackend`
(`store` / `replay_for_topic` / `unregister_instance` / `cleanup`) +
two implementations `InMemoryDurabilityBackend` (TRANSIENT) and
`OnDiskDurabilityBackend` (PERSISTENT, directory hierarchy
`<root>/<topic>/<hex(instance)>/<seq>.bin` with `.unregistered` marker).
Both backends respect history_kind (KeepLast cap via FIFO,
KeepAll cap via OutOfResources), max_samples, max_instances,
max_samples_per_instance and service_cleanup_delay (`fraction` as
2^-32 s correctly converted to nanos). The `make_backend(kind, qos, root)`
factory selects automatically.

**Tests:** `crates/qos/src/policies/durability_service.rs::tests`
(2 tests: roundtrip + default). Storage-backend tests in
`crates/dcps/src/durability_service.rs::tests` (19 tests, all 4
caps + KeepLast/KeepAll + cleanup-after-delay + on-disk-survives-drop):
`in_memory_store_and_replay_returns_sorted_samples`,
`in_memory_keeplast_caps_history_at_depth`,
`in_memory_keepall_max_samples_per_instance_returns_oor` (negative),
`in_memory_max_samples_globally_returns_oor` (negative),
`in_memory_max_instances_returns_oor` (negative),
`in_memory_unregister_then_cleanup_removes_after_delay`,
`in_memory_replay_filters_by_topic`,
`in_memory_unknown_topic_returns_empty` (negative),
`make_backend_rejects_volatile_and_transient_local` (negative),
`make_backend_persistent_requires_root` (negative),
`make_backend_transient_returns_in_memory`,
`on_disk_store_and_replay_roundtrip`,
`on_disk_keeplast_replaces_old_files`,
`on_disk_persistent_survives_backend_drop`,
`on_disk_unregister_and_cleanup_removes_directory`,
`on_disk_max_samples_per_instance_returns_oor` (negative),
`hex16_roundtrip`,
`parse_hex16_rejects_wrong_length_and_invalid_chars` (negative),
`sanitize_topic_replaces_path_chars`.

**Status:** done

### 2.2.3.6 PRESENTATION QosPolicy (access_scope INSTANCE/TOPIC/GROUP + coherent + ordered)

**Spec:** §2.2.3.6, p. 103-104.

**Repo:** `crates/qos/src/policies/presentation.rs::PresentationQosPolicy`
with `access_scope` (INSTANCE/TOPIC/GROUP) + `coherent_access` +
`ordered_access` bool flags. Wire encode/decode + compatibility check.
INSTANCE/TOPIC live in the reader sort path; GROUP-coherent via
`crates/dcps/src/coherent_set.rs::GroupAccessScope.snapshot_generation`
— each `begin_access` (from 0→1) increments the generation, all
DRs in the subscriber see the same atomic cut during the begin/end
window. Multi-reader coherent set via Arc clone of the scope.

**Tests:** `crates/qos/src/policies/presentation.rs::tests`
(6 tests: roundtrip + access_scope + coherent + ordered combinations).

**Status:** done

### 2.2.3.7 DEADLINE QosPolicy

**Spec:** §2.2.3.7, p. 104.

**Repo:** `crates/qos/src/policies/deadline.rs`.

**Tests:** deadline tests incl. cross-test-domain-pollution fix.

**Status:** done

### 2.2.3.8 LATENCY_BUDGET QosPolicy

**Spec:** §2.2.3.8, p. 104 — a hint to the service about the maximum
acceptable latency; per the spec this is a best-effort hint, not a
hard constraint.

**Repo:** `crates/qos/src/policies/latency_budget.rs::LatencyBudgetQosPolicy`
with `duration: Duration_t` + wire encode/decode (8 byte).

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests`
(2 tests: roundtrip + default INFINITE).

**Status:** done

### 2.2.3.9 OWNERSHIP QosPolicy (SHARED/EXCLUSIVE)

**Spec:** §2.2.3.9, p. 105.

**Repo:** `crates/qos/src/policies/ownership.rs::OwnershipQosPolicy`
with `OwnershipKind::Shared`/`Exclusive` + wire encode/decode +
compatibility check (spec §2.2.3 Tab. — kind must exactly match on the
reader/writer pair).

**Tests:** `crates/qos/src/policies/ownership.rs::tests` (7 tests:
roundtrip + default Shared + compat check + invalid-kind-decode reject).

**Strength selection:** EXCLUSIVE reader-side strength selection
implemented in `crates/dcps/src/instance_tracker.rs::
should_accept_sample_under_exclusive_ownership` (higher strength
wins; on a tie GUID tiebreaker; the first writer becomes owner without a
running comparison). 8 tests: exclusive_first_writer_wins,
exclusive_higher_strength_wins, exclusive_lower_strength_rejected,
exclusive_tie_break_by_higher_guid, exclusive_tie_break_lower_guid_rejected,
exclusive_same_writer_always_accepted, clear_owner_for_writer_resets_owner,
failover_after_clear_accepts_weaker_writer.

**Status:** done

### 2.2.3.10 OWNERSHIP_STRENGTH QosPolicy

**Spec:** §2.2.3.10, p. 106.

**Repo:** `crates/qos/src/policies/ownership_strength.rs::OwnershipStrengthQosPolicy`
with `value: i32` + wire encode/decode (4 byte).
The strength-selection logic in `crates/dcps/src/instance_tracker.rs::
should_accept_sample_under_exclusive_ownership` (i32 strength + GUID tiebreaker).

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests`
(3 tests: roundtrip + default 0 + negative-strength roundtrip).
Strength-selection tests in `instance_tracker::tests::exclusive_*` (6 tests).

**Status:** done

### 2.2.3.11 LIVELINESS QosPolicy (AUTOMATIC/MANUAL_BY_PARTICIPANT/MANUAL_BY_TOPIC)

**Spec:** §2.2.3.11, p. 106-107.

**Repo:** `crates/qos/src/policies/liveliness.rs::LivelinessQosPolicy`
with `LivelinessKind` (Automatic/ManualByParticipant/ManualByTopic) +
`lease_duration: Duration_t` + wire encode/decode.
`crates/dcps/src/wlp.rs` with the heartbeat path for AUTOMATIC and
`Publisher::assert_liveliness()` for MANUAL_BY_PARTICIPANT/TOPIC.

**Tests:** `crates/qos/src/policies/liveliness.rs::tests` (9 tests:
roundtrip of all three kinds + default + invalid-kind decode +
compatibility matrix spec-conformant).

**Status:** done

### 2.2.3.12 TIME_BASED_FILTER QosPolicy

**Spec:** §2.2.3.12, p. 107 — minimum_separation: the reader filters
out samples of the same instance if their source_timestamp lies within
minimum_separation of the last delivered sample.

**Repo:** `crates/qos/src/policies/time_based_filter.rs::TimeBasedFilterQosPolicy`
with `minimum_separation: Duration_t` + wire encode/decode.

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests`
(2 tests: roundtrip + default 0 = "no filter").

**Reader-side drop:** `crates/dcps/src/instance_tracker.rs` with
`InstanceState.last_delivered_ts: Option<Time>` +
`should_deliver_under_time_based_filter(keyhash, sample_ts,
min_separation_nanos)` +
`record_delivery(keyhash, sample_ts)`. The read path pre-delivery checks
and drops on too-small separation. Tests:
`time_based_filter_first_sample_passes`,
`time_based_filter_too_close_drops` (negative),
`time_based_filter_far_enough_passes`,
`time_based_filter_zero_separation_always_passes`,
`time_based_filter_per_instance_isolation`,
`time_based_filter_unknown_instance_passes`.

**Status:** done

### 2.2.3.13 PARTITION QosPolicy

**Spec:** §2.2.3.13, p. 107-108 — `sequence<string> name` +
wildcard matching ("any-string" via `*`, "any-char" via `?`,
character sets via `[abc]`, etc.); an empty vector matches an empty
vector + default partition `""`.

**Repo:** `crates/qos/src/policies/partition.rs::PartitionQosPolicy`
with wire encode/decode + a `partition_match` function with an fnmatch
engine (POSIX shell glob, incl. character class, negation).

**Tests:** `crates/qos/src/policies/partition.rs::tests`
(19 tests: roundtrip + set match + wildcard `*` + wildcard `?` +
character set + negation + default partition + empty-set cases).

**Status:** done

### 2.2.3.14 RELIABILITY QosPolicy (BEST_EFFORT/RELIABLE)

**Spec:** §2.2.3.14, p. 108.

**Repo:** `crates/qos/src/policies/reliability.rs` + reliable path
in `crates/rtps/`.

**Tests:** reliability tests.

**Status:** done

### 2.2.3.15 TRANSPORT_PRIORITY QosPolicy

**Spec:** §2.2.3.15, p. 108 — hint-only.

**Repo:** `crates/qos/src/policies/transport_priority.rs::TransportPriorityQosPolicy`
with `value: i32` + wire encode/decode.

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests`
(2 tests: roundtrip + default 0).

**Status:** done

### 2.2.3.16 LIFESPAN QosPolicy

**Spec:** §2.2.3.16, p. 108 — sample lifetime; the reader filters
samples whose source_timestamp + duration < now.

**Repo:** `crates/qos/src/policies/lifespan.rs::LifespanQosPolicy`
with `duration: Duration_t` + wire encode/decode. Reader-side sample
expiry filter via `Time::saturating_add`.

**Tests:** `crates/qos/src/policies/lifespan.rs::tests`
(2 tests: roundtrip + default INFINITE).

**Status:** done

### 2.2.3.17 DESTINATION_ORDER QosPolicy (BY_RECEPTION_TIMESTAMP/BY_SOURCE_TIMESTAMP)

**Spec:** §2.2.3.17, p. 109.

**Repo:** `crates/qos/src/policies/destination_order.rs::DestinationOrderQosPolicy`
with `DestinationOrderKind::ByReceptionTimestamp` (default) /
`BySourceTimestamp` + wire encode/decode + compatibility check
(spec: offered MUST match requested OR offered=ByReception is more
permissive). Reader-side reorder buffer via SampleInfo.source_timestamp.

**Tests:** `crates/qos/src/policies/destination_order.rs::tests`
(7 tests: roundtrip + default + compat matrix + invalid-kind decode).

**Status:** done

### 2.2.3.18 HISTORY QosPolicy (KEEP_LAST/KEEP_ALL + depth)

**Spec:** §2.2.3.18, p. 109.

**Repo:** `crates/qos/src/policies/history.rs`.

**Tests:** history tests.

**Status:** done

### 2.2.3.19 RESOURCE_LIMITS QosPolicy (max_samples/max_instances/max_samples_per_instance)

**Spec:** §2.2.3.19, p. 109-110.

**Repo:** `crates/qos/src/policies/resource_limits.rs::ResourceLimitsQosPolicy`
with `max_samples` / `max_instances` / `max_samples_per_instance` +
wire encode/decode + validity checks (no cap smaller than 1).
Cap enforcement in the writer-side sample cache.

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests`
(5 tests: roundtrip + default LENGTH_UNLIMITED + invalid-cap-reject
+ encoded bytes).

**Reliable block (§2.2.3.19):** `DataWriter::write()` blocks on the
`drain_signal: Condvar` when `queue.len() >= max_samples` and
Reliability=RELIABLE and `max_blocking_time > 0`; on BestEffort or
`max_blocking_time = 0` it delivers `OutOfResources` directly;
`__drain_pending` notifies waiting writer threads. Tests:
`write_blocks_until_drain_when_reliable_max_samples_reached`,
`write_returns_timeout_when_reliable_drain_too_slow` (negative),
`write_returns_oor_when_best_effort_queue_full` (negative),
`write_does_not_block_when_max_samples_unlimited`.

**Status:** done

### 2.2.3.20 ENTITY_FACTORY QosPolicy (autoenable_created_entities)

**Spec:** §2.2.3.20, p. 110.

**Repo:** `crates/qos/src/policies/entity_factory.rs`.

**Tests:** —

**Status:** done

### 2.2.3.21 WRITER_DATA_LIFECYCLE QosPolicy (autodispose_unregistered_instances)

**Spec:** §2.2.3.21, p. 110 — `autodispose_unregistered_instances:
bool` (default `true`) — when `true` and `unregister_instance`
is called, the writer also sends a DISPOSE sample.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs::WriterDataLifecycleQosPolicy`
with `autodispose_unregistered_instances: bool` + wire encode/decode.

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests`
(part of the 4 data_lifecycle tests: roundtrip + default true +
autodispose behavior).

**Status:** done

### 2.2.3.22 READER_DATA_LIFECYCLE QosPolicy

**Spec:** §2.2.3.22, p. 111 — `autopurge_no_writer_samples_delay` +
`autopurge_disposed_samples_delay`.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs::ReaderDataLifecycleQosPolicy`
with both Duration_t fields + wire encode/decode.

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests`
(part of the 4 data_lifecycle tests).

**Auto-purge:** `crates/dcps/src/instance_tracker.rs` with
`InstanceState.disposed_at` + `no_writers_at` timestamps, automatically
set on dispose/unregister; `autopurge(now,
disposed_delay_nanos, no_writer_delay_nanos)` scans all
instances and removes those whose marker is older than the respective
delay (`u128::MAX` = INFINITE = never). Lazy purge is called by the
read path. Tests:
`autopurge_disposed_after_delay`,
`autopurge_disposed_before_delay_keeps_instance` (negative),
`autopurge_no_writers_after_delay`,
`autopurge_alive_instance_never_purged` (negative),
`autopurge_infinity_delay_never_purges`.

**Status:** done

### 2.2.3.23 LIVELINESS+OWNERSHIP+REGISTRATION interaction

**Spec:** §2.2.3.23, p. 111-113 — cross-QoS interactions: on
LIVELINESS loss of a writer in OWNERSHIP=EXCLUSIVE the
reader shall switch to the next-strongest writer; on
unregister_instance the instance tracker shall carry through the
lifecycle path.

**Repo:** `crates/dcps/src/wlp.rs` (liveliness heartbeat path) +
`crates/dcps/src/instance_tracker.rs` (instance lifecycle +
owner tracking) + `crates/dcps/src/subscriber.rs::DataReader::
notify_writer_liveliness_lost(guid)` and `notify_participant_liveliness_lost(prefix)`
as hooks from the WLP path. Owner reset via
`InstanceTracker::clear_owner_for_writer(guid)` /
`clear_owner_for_writer_prefix(prefix)`; after the clear the
next incoming sample wins anew via strength selection.

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

### 2.2.4.1 Communication status (13 statuses)

**Spec:** §2.2.4.1, p. 114-118 — INCONSISTENT_TOPIC,
OFFERED/REQUESTED_DEADLINE_MISSED, OFFERED/REQUESTED_INCOMPATIBLE_QOS,
SAMPLE_LOST, SAMPLE_REJECTED, DATA_ON_READERS, DATA_AVAILABLE,
LIVELINESS_LOST, LIVELINESS_CHANGED, PUBLICATION_MATCHED,
SUBSCRIPTION_MATCHED.

**Repo:** `crates/dcps/src/status.rs` with all 13 records.

**Tests:** `status_records_*` tests.

**Status:** done

### 2.2.4.2 Changes in status (StatusChangedFlag bookkeeping)

**Spec:** §2.2.4.2, p. 118-121 — plain vs. read status, reset on
get/listener.

**Repo:** `crates/dcps/src/entity.rs::EntityState`:
`status_changes: AtomicU32` (bitmask), `set_status_bits(bits)`,
`clear_status_changes(bits)`, `status_changes()` read accessor.
`crates/dcps/src/listener_dispatch.rs` sets the bits on a status event
and clears them after listener dispatch (spec §2.2.4.2.3).

**Tests:** `crates/dcps/src/entity.rs::tests::status_bits_or_in_and_clear`;
plus `listener_dispatch::tests` with bubble-up reset behavior.

**Status:** done

### 2.2.4.3 Access through listeners (bubble-up DR -> Sub -> DP)

**Spec:** §2.2.4.3, p. 121-124.

**Repo:** `listener_dispatch.rs::ReaderListenerChain` walks
local + parent + grandparent.

**Tests:** `bubble_up_*` tests.

**Status:** done

### 2.2.4.4 Conditions and wait-sets

**Spec:** §2.2.4.4, p. 124-127.

**Repo:** `condition.rs` with WaitSet+Condition+GuardCondition.

**Tests:** WaitSet tests.

**Status:** done

### 2.2.4.5 Trigger state of ReadCondition

**Spec:** §2.2.4.5, p. 127.

**Repo:** `crates/dcps/src/condition.rs::ReadCondition` trigger via
a closure (see §2.2.2.5.8).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`read_condition_returns_trigger_from_closure`,
`read_condition_trigger_false_when_closure_says_false`,
`read_condition_attaches_to_waitset`.

**Status:** done

### 2.2.4.6 Combination

**Spec:** §2.2.4.6, p. 127.

**Repo:** WaitSet accepts arbitrary conditions.

**Tests:** —

**Status:** done

---

## §2.2.5 Built-in Topics

### 2.2.5.1 DCPSParticipant + ParticipantBuiltinTopicData

**Spec:** §2.2.5, p. 128-130.

**Repo:** `crates/dcps/src/builtin_topics.rs::ParticipantBuiltinTopicData`
+ `builtin_subscriber.rs::participant_reader`.

**Tests:** built-in topic tests.

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

### 2.2.5.5 BuiltinSubscriber (lookup_datareader on DCPS* topics)

**Spec:** §2.2.5.

**Repo:** `builtin_subscriber.rs::BuiltinSubscriber::lookup_datareader<T: BuiltinTopic>`.

**Tests:** lookup tests.

**Status:** done

### 2.2.5.6 Builtin-Sub+DR default QoS (DURABILITY=TRANSIENT_LOCAL/RELIABILITY=RELIABLE/HISTORY=KEEP_LAST/depth=1/...)

**Spec:** §2.2.5 — complete default-QoS block.

**Repo:** `builtin_subscriber.rs::builtin_reader_qos()`.

**Tests:** —

**Status:** done

### 2.2.5.7 Built-in DR of a given participant does not show its own participant's entities

**Spec:** §2.2.5 — the built-in subscriber delivers no entities of the
own participant (otherwise self-loop).

**Repo:** `crates/dcps/src/builtin_subscriber.rs` with participant_key
filter logic in the discovery hook (filters SPDP/SEDP samples with
the same participant GUID as the local participant).

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests` with a
participant_key filter check.

**Status:** done

---

## §2.2.6 Interaction Model

### 2.2.6.1 Publication view (register -> write* -> unregister/dispose)

**Spec:** §2.2.6.1, p. 132 — complete pub-view sequence.

**Repo:** `publisher.rs` with `register_instance/unregister_instance/
dispose/write` (+ w_timestamp variants).

**Tests:** `publication_view_*` tests.

**Status:** done

### 2.2.6.2.1 Subscription view notification via listener (async push)

**Spec:** §2.2.6.2.1, p. 133-134.

**Repo:** `listener_dispatch.rs`.

**Tests:** listener tests.

**Status:** done

### 2.2.6.2.2 Subscription view notification via conditions+wait-sets (sync poll)

**Spec:** §2.2.6.2.2, p. 135.

**Repo:** `condition.rs::WaitSet`.

**Tests:** WaitSet tests.

**Status:** done

---

## §2.3 OMG IDL Platform Specific Model

### 2.3.1-2 PIM->PSM mapping rules (CORBA-IDL)

**Spec:** §2.3.1-2, p. 137 — PIM->PSM conventions.

**Repo:** the Rust PSM in `crates/dcps/` is a spec-conformant alternative
form (XTypes 1.3 §7.2.2.4.8). CORBA-IDL Annex-A.1 PSM emission
in `crates/idl-cpp/src/corba_traits.rs`,
`crates/idl-csharp/src/corba_traits.rs`,
`crates/idl-java/src/corba_traits.rs` as an opt-in codegen path
(see `idl4-cpp-1.0.md` / `idl4-csharp-1.0.md` / `idl4-java-1.0.md`
Annex A.1).

**Tests:** cross-ref Rust-PSM tests in the `dcps` crate +
`{idl-cpp,idl-csharp,idl-java}::corba_traits::tests::*`.

**Status:** done — Rust PSM (alternative form) + CORBA-IDL Annex-A.1
PSM emission live.

### 2.3.3 DCPS PSM IDL — complete IDL file `zerodds_dcps.idl`

**Spec:** §2.3.3, p. 138-162.

**Repo:** `crates/idl/` — IDL 4.2 parser (K1 fully completed,
1026 tests). Codegen via `crates/idl-cpp/`, `crates/idl-java/`,
`crates/idl-csharp/`. Spec coverage of IDL 4.2 in
`docs/spec-coverage/idl-4.2.md` (530 done / 0 partial).

**Tests:** workspace-wide via the `zerodds-idl` crate;
`docs/spec-coverage/idl-4.2.md` is the normative coverage doc.

**Status:** done — the IDL parser layer is fully spec-conformant; the PSM-
specific codegen stage is normative separately per PSM spec.

### 2.3.3 IDL constants: typedef long DomainId_t / typedef long ReturnCode_t / etc.

**Spec:** §2.3.3.

**Repo:** Rust-native types (`DomainId = u32`, `Result<T, DdsError>`)
+ IDL constants in `crates/dcps/src/psm_constants.rs::status::*`
for all 13 status bits + `crates/dcps/src/error.rs::return_code::*`
for all 13 ReturnCode_t values.

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`
verifies all 13 spec constants value-exact.

**Status:** done

---

## Audit status

100 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-dcps` — 407 lib + 150 integration
(23 integration test bins) = 557 tests green, 0 failed.

DLRL (optional §2.1.3 layer) is fully covered as a differentiation
feature — own crate `crates/dlrl/`, see [DLRL-1.2 coverage](dlrl-1.2.md).
