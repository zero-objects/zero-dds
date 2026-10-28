# OMG DDS 1.2 — Data Local Reconstruction Layer (DLRL) — Spec Coverage

**Spec:** [OMG DDS v1.2 — formal/07-01-01, January 2007 →](https://www.omg.org/spec/DDS/1.2/) (393 pp.). DLRL is in
**§8 Data Local Reconstruction Layer** (pp. 173-211) and **Annex B Syntax
for DLRL Queries and Filters** (p. 245).

**Note on spec versioning:** DDS 1.4 (`formal/15-04-10`) and following
revisions split out the DCPS part (`zerodds-dcps-1.4.md`); the DLRL
specification was not carried into 1.4 and thus remains, as DDS 1.2 §8, the
authoritative normative source for DLRL.

**Context.** DLRL is the optional object-oriented layer above DCPS (the
DDS-topic world → the domain-object world). ZeroDDS implements a **subset
variant** that covers the core concepts (ObjectCache, Relationship, Query,
Subscription, Transaction) but does not expose the full spec hierarchy of
the ~16 DLRL entity classes (CacheFactory, CacheBase, Cache, CacheAccess,
ObjectHome, Selection, etc.) as standalone classes. This is a deliberate
simplification for the migration path of older DDS-1.x applications with
`pragma DLRL` IDL — many spec classes are only indirectly used there.
Spread across:

- `crates/dlrl/` — DLRL subset runtime (ObjectCache, Relationship, Query, Subscription, Transaction, pragma parsing)
- `crates/dlrl-codegen/` — code generation for the Generation Process (§8.2.2)
- `tools/idlc/src/dlrl.rs` — CLI front end (`zerodds-idlc dlrl`), wiring
  `dlrl-codegen` in as a subcommand; see §8.2.2 for the pragma-name
  collision with OpenDDS's `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` dialect

**Crate mapping:**

| Spec area | Crate / module |
|---|---|
| §8.1.3.1 DLRL objects | `crates/dlrl/src/object_cache.rs` |
| §8.1.3.2 Relations | `crates/dlrl/src/relationship.rs` |
| §8.1.4 Structural Mapping (subset) | `crates/dlrl/src/object_cache.rs` + `subscription.rs` |
| §8.1.5 Operational Mapping | `crates/dlrl/src/transaction.rs` |
| §8.1.6 Functional Mapping (listener path) | `crates/dlrl/src/subscription.rs` |
| §8.2 PSM (IDL pragma DLRL) | `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (codegen) | `crates/dlrl-codegen/`, CLI: `tools/idlc/src/dlrl.rs` |
| Annex B Query/Filter Syntax | `crates/dlrl/src/query.rs` |

---

## §8.1 Platform Independent Model (PIM)

### §8.1.1 Overview and design rationale

**Spec:** §8.1.1, p. 173 (PDF) — "The purpose of this layer is to provide
more direct access to the exchanged data, seamlessly integrated with the
native-language constructs. Object orientation has been selected for all the
benefits it provides in software engineering. As for DCPS, typed interfaces
have been selected, for the same reasons of ease of use and potential
performance. As far as possible, DLRL is designed to allow the application
developer to use the underlying DCPS features."

**Repo:** `crates/dlrl/src/lib.rs` (module doc).

**Tests:** —

**Status:** n/a (informative)

### §8.1.2 DLRL description

**Spec:** §8.1.2, p. 173 (PDF) — "With DLRL, the application developer will
be able to: Describe classes of objects with their methods, data fields and
relations; Attach some of those data fields to DCPS entities; Manipulate
those objects (i.e., create, read, write, delete) using the native language
constructs that will, behind the scenes, activate the attached DCPS entities
in the appropriate way; Have those objects managed in a cache of objects,
ensuring that all the references that point to a given object actually point
to the same language cell."

**Repo:** conceptually covered by `object_cache.rs`, `subscription.rs`,
`relationship.rs`.

**Tests:** —

**Status:** n/a (informative) — description; concrete requirements in
§8.1.3-§8.1.6.

## §8.1.3 What can be modeled with DLRL

### §8.1.3.1 DLRL objects

**Spec:** §8.1.3.1, p. 174 (PDF) — DLRL objects with `methods` and
`attributes` (local vs. shared); shared attributes can be mono-valued or
multi-valued (List/Map/Set). "Object identity is given by an `oid` (object
ID) part of any DLRL object."

**Repo:** `crates/dlrl/src/object_cache.rs::{ObjectId, ObjectState,
ObjectRef, WeakObjectRef, ObjectCache}`.

**Tests:** `crates/dlrl/src/object_cache.rs::tests::{register_then_get_round_trip,
ids_returns_stable_order, mark_deleted_then_commit_removes,
mark_deleted_unknown_returns_false,
re_register_increments_version_and_marks_modified,
rollback_after_commit_restores_modified_to_committed,
rollback_drops_new_objects, weak_ref_invalidated_on_modify,
weak_ref_resolves_at_same_version}` (9).

**Status:** done — object identity (oid) + lifecycle
(created/modified/deleted) + cache pinning + all 17 entity classes
(Collection/List/Set/StrMap/IntMap incl.) declared as
`crates/dlrl/src/metamodel.rs::DlrlEntityKind`.

### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, p. 175 (PDF) — "Single inheritance is allowed between
DLRL objects. Any object inheriting from a DLRL object is itself a DLRL
object. **ObjectRoot** is the ultimate root for all DLRL objects."

**Repo:** `crates/dlrl/src/metamodel.rs::ObjectRoot` trait with
`oid/repository_id/is_modified/is_deleted` operations as the common base
class for all DLRL objects.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::object_root_trait_callable`.

**Status:** done

### §8.1.3.2.2 Associations

**Spec:** §8.1.3.2.2, p. 175 (PDF) — "Supported association ends are either
`to-1` or `to-many`. [...] Plain use-relations (no impact on the object
life-cycle); Compositions (constituent object lifecycle follows the compound
object's one). Couples of relations can be managed consistently (one being
the inverse of the other), to make a real association (in the UML sense):
One plain relation can inverse another plain relation, providing that the
types match: can make 1-1, 1-n, n-m. One composition relation can only
inverse a to-1 relation to the compound object: can make 1-1 or 1-n."

**Repo:** `crates/dlrl/src/relationship.rs::{RelationshipKind, Direction,
CascadeMode, Relationship, RelationshipResolver}` with to-1/to-many +
cascade-update/delete + bidirectional inverses.

**Tests:** `crates/dlrl/src/relationship.rs::tests::{mono_adds_one_entry,
bi_adds_inverse, relationship_kind_distinct, cascade_modes_distinct,
cascade_delete_targets_only_marked_relations,
cascade_update_targets_only_marked_relations,
empty_resolver_returns_empty_lists}` (7).

**Status:** done

### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, p. 175-176 (PDF, with Figure 8.1) — UML metamodel with
classes `Class` (final:Boolean), `Relation` (is_composition:Boolean),
`MultiRelation`, `MonoRelation`, `Attribute`, `MultiAttribute`,
`MonoAttribute`, `MultiRefType`, `MultiSimpleType`, `SimpleType`
(BasicType/EnumerationType/SimpleStructType), `CollectionBase`
(SetBase/ListBase/MapBase), `ObjectRoot`, `ObjectHome`. BasicType instances:
long/short/char/octet/real/double/string/sequence-of-any.

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel, ObjectRoot,
DlrlEntityKind}`; the relation side additionally in
`crates/dlrl/src/relationship.rs::{RelationshipKind, Relationship}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{class_metamodel_default_fields,
multi_attribute_metamodel_construct, mono_relation_metamodel_construct,
object_root_trait_callable, dlrl_entity_kinds_count_17}`;
`crates/dlrl/src/relationship.rs::tests::relationship_kind_distinct`.

**Status:** done — the metamodel classes (Class/MultiAttribute/MonoRelation/
ObjectRoot) carry the fields of Figure 8.1.

## §8.1.4 Structural Mapping

### §8.1.4.1 Design principles

**Spec:** §8.1.4.1, p. 177 (PDF) — design principles: no unnecessary data
duplication, do not impede efficiency, flexible mapping per attribute,
default mapping available.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §8.1.4.2 Mapping rules

**Spec:** §8.1.4.2, p. 177-178 (PDF) — three rule sets:

* §8.1.4.2.1 Mapping of Classes — "Each DLRL class is associated with at
  least one DCPS table" (p. 177).
* §8.1.4.2.2 Mapping of an Object Reference — "full oid" as class name +
  oid number (p. 177-178).
* §8.1.4.2.3 Mapping of Attributes and Relations — mono/multi attribute onto
  DCPS cells; the Map/List/Set index-field pattern (p. 178).

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel}` carries the three rule
sets: `main_topic` (class -> DCPS table), `class_field` + `oid_field` +
`full_oid_required` (object reference as a full oid), `topic` +
`target_field`/`target_fields` + `key_fields` + `index_field` (attributes
and relations onto DCPS cells); `crates/dlrl/src/object_cache.rs::ObjectId`
is the run-time side of the oid.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::mapping_rules_bind_class_reference_and_attributes_per_spec_8_1_4_2`
— checks all three rule sets in one pass (a class names one topic, the full
oid is the class field plus the oid field, a multi-attribute lives in its own
topic with an index field and the owner's key fields). In addition
`crates/dlrl/src/metamodel.rs::tests::multi_attribute_metamodel_construct`,
`crates/dlrl/src/metamodel.rs::tests::mono_relation_metamodel_construct`.

**Status:** done

### §8.1.4.3 Default mapping

**Spec:** §8.1.4.3, p. 179 (PDF) — DCPS topic name = DLRL class name; oid
field names = `class`, `oid`; multi-attribute topic = `<Class>.<attribute>`;
index field = `index`.

**Repo:** `crates/dlrl/src/metamodel.rs::default_mapping::{topic_name,
multi_topic, DEFAULT_OID_FIELD, DEFAULT_CLASS_FIELD, DEFAULT_INDEX_FIELD}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{default_mapping_topic_name_uses_class,
default_mapping_multi_topic_dot_separated,
default_mapping_constants_match_spec}` (3).

**Status:** done

### §8.1.4.4 Metamodel with mapping information

**Spec:** §8.1.4.4, p. 180-181 (PDF, with Figure 8.2) — extension of the
§8.1.3.3 metamodel classes with mapping fields:

* §8.1.4.4.1 Class — `main_topic`, `oid_field`, `class_field`,
  `full_oid_required`, `final`.
* §8.1.4.4.2 MonoAttribute — `topic`, `target_field`, `key_fields[*]`.
* §8.1.4.4.3 MultiAttribute — like MonoAttribute + `index_field`.
* §8.1.4.4.4 MonoRelation — `topic`, `target_fields[*]`, `key_fields[*]`,
  `full_oid_required`, `is_composition`.
* §8.1.4.4.5 MultiRelation — like MonoRelation + `index_field`.

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel}` with exactly these mapping
fields; their tag source in `crates/dlrl/src/pragma.rs::{DlrlPragma,
parse_pragma}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{class_metamodel_default_fields,
multi_attribute_metamodel_construct, mono_relation_metamodel_construct}`;
`crates/dlrl/src/pragma.rs::tests::{parses_relation_pragma,
parses_data_key_pragma}`.

**Status:** done

### §8.1.4.5 Mapping when DCPS model is fixed

**Spec:** §8.1.4.5, p. 182 (PDF) — DLRL against an existing DCPS model for
legacy applications.

**Repo:** `crates/dlrl/src/metamodel.rs::ClassMetamodel` (`main_topic`,
`oid_field`, `class_field` freely assignable instead of the default
convention); `crates/dlrl/src/pragma.rs::parse_pragma` reads the foreign
names from `#pragma DLRL data_type`/`data_key`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::class_metamodel_over_a_fixed_dcps_model_keeps_the_foreign_names`;
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma}`.

**Status:** done

### §8.1.4.6 How is this mapping indicated?

**Spec:** §8.1.4.6, p. 182-183 (PDF, with Figure 8.3) — the DLRL generator
with a model description + model tags as input; output: native model
description, dedicated DLRL entities, DCPS description.

**Repo:** model tags as `#pragma DLRL` lines in
`crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}`; the generator in
`crates/dlrl-codegen/src/lib.rs::collect_type_infos` (model description out of the tags)
and the four language backends
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`.

**Tests:** tags as generator input:
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma, parses_relation_pragma}`; generator output from those
tags:
`crates/dlrl-codegen/src/lib.rs::tests::{everything_about_one_type_lands_in_one_entry,
every_backend_carries_every_key_and_relation, every_home_names_its_object_class}`.

**Status:** done — pragma input + codegen output match the generator
pattern.

## §8.1.5 Operational Mapping

### §8.1.5.1 Attachment to DCPS entities

**Spec:** §8.1.5.1, p. 183 (PDF) — a DLRL class is connected to multiple
DCPS topics (DataWriter/DataReader); all DataWriters/DataReaders of a DLRL
object are attached to a single Publisher/Subscriber. "DLRL has attached a
Publisher and/or a Subscriber to the notion of a Cache object."

**Repo:** `crates/dlrl/src/metamodel.rs::CacheAttachmentState` (attachment
state of the cache to publisher/subscriber);
`crates/dlrl/src/subscription.rs::SubscriptionRegistry` (topic -> listener
binding, the reader side of the attachment).

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`;
`crates/dlrl/src/subscription.rs::tests::{home_listener_registered_per_topic,
fanout_invokes_listeners_for_matching_topic, fanout_ignores_other_topics}`.

**Status:** done — cache attachment states + topic-bound listener binding
declared.

### §8.1.5.2 Creation of DCPS entities

**Spec:** §8.1.5.2, p. 183 (PDF) — operations to create + activate the DCPS
entities, bound to the cache.

**Repo:** `crates/dlrl/src/metamodel.rs::{CacheMode, CacheAttachmentState}`
(creation and activation states);
`crates/dlrl/src/object_cache.rs::ObjectCache::{register, mark_deleted,
commit_all}` (object lifecycle at the cache).

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_modes_distinct`,
`crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`,
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/object_cache.rs::tests::mark_deleted_then_commit_removes`.

**Status:** done — the cache-bound lifecycle is declared.

### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, p. 183 (PDF) — QoS settable per DCPS entity; DLRL
provides the means to find the DCPS entities from the DLRL entities so the
application developer can set QoS.

**Repo:** `crates/dlrl/src/metamodel.rs::{CacheMode, CacheAttachmentState}`
— through the attachment state the application finds the DCPS entities on
which it sets QoS; the QoS values themselves live in `crates/dcps/`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`,
`crates/dlrl/src/metamodel.rs::tests::cache_modes_distinct`.

**Status:** done — finding the DCPS entities through the attachment state is
declared; the QoS values themselves are a DCPS matter.

## §8.1.6 Functional Mapping

### §8.1.6.1.1 DLRL requested functions — publishing application

**Spec:** §8.1.6.1.1, p. 184 (PDF) — the publishing app should
create/modify/destroy objects, send publication requests, manage concurrent
modifications consistently.

**Repo:** `crates/dlrl/src/object_cache.rs::ObjectCache`
(register/mark_deleted/commit_all/rollback_all),
`crates/dlrl/src/transaction.rs` (concurrent-mod handling).

**Tests:** create/modify/delete:
`crates/dlrl/src/object_cache.rs::tests::{register_then_get_round_trip,
re_register_increments_version_and_marks_modified,
mark_deleted_then_commit_removes, rollback_drops_new_objects}`; consistent
handling of concurrent modifications:
`crates/dlrl/src/transaction.rs::tests::{commit_without_concurrent_modify_succeeds,
commit_detects_optimistic_conflict, rollback_resets_state,
double_commit_fails, commit_after_rollback_fails,
each_transaction_gets_unique_id}`.

**Status:** done — publication lifecycle + conflict detection on concurrent
modification are declared.

### §8.1.6.1.2 DLRL requested functions — subscribing application

**Spec:** §8.1.6.1.2, p. 184-185 (PDF) — loading objects, reading
attributes/relations, navigation, awareness of object changes. Incl. implicit
vs. explicit subscriptions, cache management, user interaction.

**Repo:** `crates/dlrl/src/subscription.rs::{HomeFactory,
SubscriptionRegistry, HomeListener, ObjectListener}`.

**Tests:** `crates/dlrl/src/subscription.rs::tests::{home_listener_registered_per_topic,
object_listener_only_fires_for_its_own_id,
registry_notifies_both_home_and_object_listeners,
fanout_invokes_listeners_for_matching_topic, fanout_ignores_other_topics}`
(5).

**Status:** done — loading, navigation and change notification through home
and object listeners are declared.

### §8.1.6.1.3 DLRL requested functions — publishing and subscribing applications

**Spec:** §8.1.6.1.3, p. 185 (PDF) — mixed publish+subscribe in one app.

**Repo:** implicit via the combination §8.1.6.1.1+§8.1.6.1.2.

**Tests:** `crates/dlrl/src/subscription.rs::tests::one_application_publishing_and_subscribing_sees_its_own_changes`;
`crates/dlrl/src/object_cache.rs::tests::re_register_increments_version_and_marks_modified`,
`crates/dlrl/src/object_cache.rs::tests::mark_deleted_then_commit_removes`.

**Status:** done — one application that publishes and subscribes sees its own
changes through the same cache.

### §8.1.6.2 DLRL entities

**Spec:** §8.1.6.2, p. 185-188 (PDF, with Figure 8.4 + table p. 187) — 17
DLRL entity classes:

1. **CacheFactory** — singleton for cache creation.
2. **CacheBase** — abstract base for all cache types.
3. **Cache** — set of locally-available objects.
4. **CacheAccess** — read/write mode for an object subset.
5. **CacheListener** — interface for incoming cache updates.
6. **Contract** — defines which objects are copied from the cache to the
   CacheAccess.
7. **ObjectHome** — manager for all instances of an app class.
8. **ObjectListener** — interface for incoming ObjectHome updates.
9. **Selection** — a subset of objects via an expression.
10. **SelectionCriterion** — a filter criterion for a Selection.
11. **FilterCriterion** — user-defined filter (specialization).
12. **QueryCriterion** — SQL-query-based filter (specialization).
13. **SelectionListener** — interface for Selection updates.
14. **ObjectRoot** — abstract root for all app classes.
15. **Collection** — abstract root for collections.
16. **List, Set, StrMap, IntMap** — collection specializations.

Plus 8 exceptions: DCPSError, BadHomeDefinition, NotFound, AlreadyExisting,
AlreadyDeleted, PreconditionNotMet, NoSuchElement, SQLError.

**Repo:** subset implementation: `ObjectCache` (≈Cache), `HomeFactory`
(≈ObjectHome+CacheFactory mix), `HomeListener`/`ObjectListener`
(≈CacheListener/ObjectListener), `Query` (≈Selection+QueryCriterion mix).
Missing as standalone types: `CacheFactory`, `CacheBase`, `CacheAccess`,
`Contract`, `Selection`, `SelectionCriterion`, `FilterCriterion`,
`SelectionListener`, `ObjectRoot`, `Collection`/`List`/`Set`/`StrMap`/`IntMap`.
Plus all 8 exception types missing.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kinds_count_17,
dlrl_entity_kind_spec_names_match, dlrl_entity_kinds_distinct,
dlrl_exception_repository_ids_distinct,
dlrl_exception_repo_id_omg_namespace}` — the 17 entity classes and the 8
exception types are declared by name and repository ID; the subset types
themselves:
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`,
`crates/dlrl/src/query.rs::tests::topic_filter_narrows_result`.

**Status:** partial — complete by name (`DlrlEntityKind` with 17 values,
`DlrlException` with 8), but as standalone types only the subset
`ObjectCache`/`HomeFactory`/`HomeListener`/`ObjectListener`/`Query`.
`CacheFactory`, `CacheBase`, `CacheAccess`, `Contract`, `Selection`,
`SelectionCriterion`, `FilterCriterion`, `SelectionListener`,
`Collection`/`List`/`Set`/`StrMap`/`IntMap` are missing as types — see the
following `open` item.

### §8.1.6.2 (rest) DLRL entity classes as standalone types

**Spec:** §8.1.6.2, p. 185-188 (PDF, Figure 8.4) — the 13 entity classes
`CacheFactory`, `CacheBase`, `CacheAccess`, `Contract`, `Selection`,
`SelectionCriterion`, `FilterCriterion`, `SelectionListener`, `Collection`,
`List`, `Set`, `StrMap`, `IntMap` as standalone classes with their
attributes.

**Repo:** —

**Tests:** —

**Status:** open

### §8.1.6.3 Details on DLRL entities

**Spec:** §8.1.6.3, p. 188-210 (PDF) — a table with attributes + operations
per entity class. Sub-sections §8.1.6.3.1-§8.1.6.3.17 (one per entity class).

**Repo:** operations subset via `crates/dlrl/src/subscription.rs` (listener
callbacks), `crates/dlrl/src/object_cache.rs` (cache operations) and
`crates/dlrl/src/query.rs` (selection-like queries); the exception types of
the tables in `crates/dlrl/src/metamodel.rs::DlrlException`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kinds_count_17,
dlrl_entity_kind_spec_names_match, dlrl_entity_kinds_distinct,
dlrl_exception_repository_ids_distinct,
dlrl_exception_repo_id_omg_namespace}`;
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`.

**Status:** partial — the exception types and entity names are complete, the
operations only a subset. Signature conformance for the tables
§8.1.6.3.1-§8.1.6.3.17 is not established — see the following `open` item.

### §8.1.6.3 (rest) Method signatures of the entity tables

**Spec:** §8.1.6.3, p. 188-210 (PDF) — one table of attributes and operations
per entity class; §8.1.6.3.1-§8.1.6.3.17 give the signatures.

**Repo:** —

**Tests:** —

**Status:** open

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time entities

**Spec:** §8.2.1, p. 211-228 (PDF) — the IDL definition of all entity
interfaces + a service bootstrap (`DLRL_initialize`).

**Repo:** the run-time entities exist as a Rust API in
`crates/dlrl/src/object_cache.rs`, `crates/dlrl/src/subscription.rs`,
`crates/dlrl/src/query.rs` and `crates/dlrl/src/transaction.rs`; the
repository IDs of the PSM types in
`crates/dlrl/src/metamodel.rs::{DlrlEntityKind, DlrlException}`. There is no
1:1 re-emission of the §8.2.1 IDL interfaces or the `DLRL_initialize`
bootstrap.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kind_spec_names_match,
dlrl_exception_repo_id_omg_namespace}` — the PSM names and the OMG
repository-ID namespace match the IDL definition; the run-time side:
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`,
`crates/dlrl/src/transaction.rs::tests::commit_path_no_conflict`.

**Status:** partial — the PSM names and repository IDs match and the run-time
operations exist as a Rust API. The IDL interface definitions and the
`DLRL_initialize` bootstrap are not emitted — see the following `open` item.

### §8.2.1 (rest) IDL PSM emission + DLRL_initialize

**Spec:** §8.2.1, p. 211-228 (PDF) — the IDL definition of all entity
interfaces plus the service bootstrap `DLRL_initialize`.

**Repo:** —

**Tests:** —

**Status:** open

### §8.2.2 Generation process

**Spec:** §8.2.2, p. 228-234 (PDF) — `pragma DLRL`-annotated IDL → PSM
classes (Object/Home/Selection templates).

**Repo:** `crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}` (pragma
lexer), `crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs` (per language
backend `generate_*_object` + `generate_*_home`). CLI front end:
`tools/idlc/src/dlrl.rs` (subcommand `zerodds-idlc dlrl --lang
{cpp,csharp,java,typescript}`) — scans the raw IDL text directly through
`zerodds_dlrl::pragma::parse_pragma`, independently of the AST-facet path
`idl-lower` reads `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` through for OpenDDS's
dialect (`crates/idl-lower/src/lower.rs`, whose diagnostic names both
readings).

**Tests:** pragma parser:
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma, parses_relation_pragma, extra_whitespace_tolerated,
data_type_with_extra_tokens_rejected, data_key_with_one_token_rejected,
missing_quotes_rejected, non_dlrl_line_rejected, unknown_tag_rejected}` (9).
Codegen:
`crates/dlrl-codegen/src/lib.rs::tests::{everything_about_one_type_lands_in_one_entry,
key_order_is_the_order_the_pragmas_came_in,
a_key_without_its_data_type_still_creates_the_entry,
the_class_name_is_the_last_segment,
every_backend_carries_every_key_and_relation,
every_home_names_its_object_class,
every_backend_spells_the_key_type_it_is_given,
every_backend_carries_every_attribute_with_a_getter_and_a_setter,
every_backend_carries_structured_and_multi_valued_attributes}` (9) —
the last five exercise all four language backends (`cpp`, `csharp`,
`java`, `ts`) in one pass. CLI: `tools/idlc/tests/dlrl.rs` (20 tests —
generation per language, the Java object/home file split, the pragma
collision with OpenDDS in one file and each alone, a malformed pragma as
a diagnostic, `#include`, `--lang all`, the build contract, key types
from the IDL, attributes, structured and multi-valued attributes, the
refusal of a key or attribute without a mapping);
`tools/idlc/tests/dlrl_compiles.rs` (8 tests — the output compiles with
`c++`, `dotnet`, `javac` and `tsc` against stubs of the missing runtime
types, once alone and once together with the `idlc generate` output for
the same IDL and the wire runtime from `runtime/`; all eight run on
the Linux test host); further CLI parser tests
in `tools/idlc/src/main.rs`. Collision diagnostic:
`crates/idl-lower/tests/dialects.rs` (4 tests).

Every member of the data type that is neither a key nor a relation is an
attribute of the object class (§8.1.2.2, §8.1.4.3). Before, the
generator wrote only keys and relations.

- Mono-valued, with a getter and a setter (§8.2.3.5 sets them through
  the object, `t1->x(1000.0)`): a basic type, an enumeration or a simple
  structure. Simple means every member is a basic type, an enumeration,
  a simple structure or a fixed array of those, so a fixed number of
  DCPS fields (§8.1.4.2.3, footnote 3 to §8.1.2.2).
- Multi-valued, with a getter and no setter (a collection changes
  through its own `add`/`put`/`remove`, §8.1.6.3.16–19): a `sequence` or
  a one-dimensional array as a List, `map<string, T>` as a StrMap,
  `map<integer type, T>` as an IntMap; the items as for mono-valued.
- An enumeration or a structure carries the name `idlc generate` gives
  it in the same language: C++ `::demo::Money`, Java
  `ZdTypes.demo.Money` (an enumeration as `int`), C#
  `global::demo.Money`, TypeScript `demo.Money`. The C++ file then
  includes `<stem>.hpp`, the TypeScript file imports from `./<stem>.js`.
- Keys accept enumerations and simple structures too.
- A member without a mapping fails the run with its name and the reason:
  a union, a nested collection, a structure containing a collection, a
  multi-dimensional array, a map with another key type, a wide string.

Mapping to DCPS tables (the main table, a topic `<class>.<attribute>`
with `index` per multi-valued attribute, §8.1.4.3) is the work of a DLRL
runtime; ZeroDDS ships none for C++, C#, Java or TypeScript, and the
generator writes the class shape.

**Status:** done

### §8.2.3 Example

**Spec:** §8.2.3, p. 234-244 (PDF) — illustrative example code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## Annex B

### Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, p. 245-249 (PDF) — the BNF grammar for DLRL Selection
filter expressions (an SQL subset analogous to the DCPS ContentFilteredTopic).

**Repo:** `crates/dlrl/src/query.rs::{Query, SortOrder, QueryError}`.

**Tests:** `crates/dlrl/src/query.rs::tests::{empty_query_returns_all,
topic_filter_narrows_result, state_filter_only_returns_matching,
custom_filter_applied, limit_caps_result_size, limit_too_large_rejected,
order_by_sorts_ascending, order_by_descending_reverses,
empty_topic_rejected}` (9);
`crates/dlrl/src/metamodel.rs::tests::query_expression_construct`.

**Status:** done — the filter, sort and limit semantics of the query subset
are declared.

---

## Audit status

17 done / 3 partial / 3 open / 4 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-dlrl -p zerodds-dlrl-codegen` —
`zerodds-dlrl` 66 tests green, `zerodds-dlrl-codegen` 6 tests green,
0 failed. CLI front end: `cargo test -p zerodds-idlc --test dlrl` — 15
tests green; pragma collision diagnostic: `cargo test -p
zerodds-idl-lower --test dialects` — 59 tests green (4 of them about
the collision).

Open items and partial implementations: `dlrl-1.2.open.en.md`.
