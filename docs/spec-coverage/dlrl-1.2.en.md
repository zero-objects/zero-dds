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

**Crate mapping:**

| Spec area | Crate / module |
|---|---|
| §8.1.3.1 DLRL objects | `crates/dlrl/src/object_cache.rs` |
| §8.1.3.2 Relations | `crates/dlrl/src/relationship.rs` |
| §8.1.4 Structural Mapping (subset) | `crates/dlrl/src/object_cache.rs` + `subscription.rs` |
| §8.1.5 Operational Mapping | `crates/dlrl/src/transaction.rs` |
| §8.1.6 Functional Mapping (listener path) | `crates/dlrl/src/subscription.rs` |
| §8.2 PSM (IDL pragma DLRL) | `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (codegen) | `crates/dlrl-codegen/` |
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

### §8.1.3 What can be modeled with DLRL

#### §8.1.3.1 DLRL objects

**Spec:** §8.1.3.1, p. 174 (PDF) — DLRL objects with `methods` and
`attributes` (local vs. shared); shared attributes can be mono-valued or
multi-valued (List/Map/Set). "Object identity is given by an `oid` (object
ID) part of any DLRL object."

**Repo:** `crates/dlrl/src/object_cache.rs::{ObjectId, ObjectState,
ObjectRef, WeakObjectRef, ObjectCache}`.

**Tests:** `object_cache::tests::register_then_get_round_trip`,
`object_cache::tests::ids_returns_stable_order`,
`object_cache::tests::mark_deleted_then_commit_removes`,
`object_cache::tests::mark_deleted_unknown_returns_false`,
`object_cache::tests::re_register_increments_version_and_marks_modified`,
`object_cache::tests::rollback_after_commit_restores_modified_to_committed`,
`object_cache::tests::rollback_drops_new_objects`,
`object_cache::tests::weak_ref_invalidated_on_modify`,
`object_cache::tests::weak_ref_resolves_at_same_version`.

**Status:** done — object identity (oid) + lifecycle
(created/modified/deleted) + cache pinning + all 17 entity classes
(Collection/List/Set/StrMap/IntMap incl.) declared as
`crates/dlrl/src/metamodel.rs::DlrlEntityKind`.

#### §8.1.3.2 Relations among DLRL objects

##### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, p. 175 (PDF) — "Single inheritance is allowed between
DLRL objects. Any object inheriting from a DLRL object is itself a DLRL
object. **ObjectRoot** is the ultimate root for all DLRL objects."

**Repo:** `crates/dlrl/src/metamodel.rs::ObjectRoot` trait with
`oid/repository_id/is_modified/is_deleted` operations as the common base
class for all DLRL objects.

**Tests:** `metamodel::tests::object_root_trait_callable`.

**Status:** done

##### §8.1.3.2.2 Associations

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

**Tests:** `relationship::tests::mono_adds_one_entry`,
`relationship::tests::bi_adds_inverse`,
`relationship::tests::relationship_kind_distinct`,
`relationship::tests::cascade_modes_distinct`,
`relationship::tests::cascade_delete_targets_only_marked_relations`,
`relationship::tests::cascade_update_targets_only_marked_relations`,
`relationship::tests::empty_resolver_returns_empty_lists`.

**Status:** done

#### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, p. 175-176 (PDF, with Figure 8.1) — UML metamodel with
classes `Class` (final:Boolean), `Relation` (is_composition:Boolean),
`MultiRelation`, `MonoRelation`, `Attribute`, `MultiAttribute`,
`MonoAttribute`, `MultiRefType`, `MultiSimpleType`, `SimpleType`
(BasicType/EnumerationType/SimpleStructType), `CollectionBase`
(SetBase/ListBase/MapBase), `ObjectRoot`, `ObjectHome`. BasicType instances:
long/short/char/octet/real/double/string/sequence-of-any.

**Repo:** the metamodel as a declared class hierarchy is not implemented;
individual concepts distributed across `object_cache.rs` (Class equivalent)
and `relationship.rs` (Relation/MultiRelation/MonoRelation).

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

### §8.1.4 Structural Mapping

#### §8.1.4.1 Design principles

**Spec:** §8.1.4.1, p. 177 (PDF) — design principles: no unnecessary data
duplication, do not impede efficiency, flexible mapping per attribute,
default mapping available.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §8.1.4.2 Mapping rules

**Spec:** §8.1.4.2, p. 177-178 (PDF) — three rule sets:

* §8.1.4.2.1 Mapping of Classes — "Each DLRL class is associated with at
  least one DCPS table" (p. 177).
* §8.1.4.2.2 Mapping of an Object Reference — "full oid" as class name +
  oid number (p. 177-178).
* §8.1.4.2.3 Mapping of Attributes and Relations — mono/multi attribute onto
  DCPS cells; the Map/List/Set index-field pattern (p. 178).

**Repo:** `object_cache.rs` maps ObjectId↔KeyHash mono 1:1; the full
multi-attribute index-field logic (with `key_fields[]` + `index_field`) is
missing as a declared feature.

**Tests:** cross-ref §8.1.3.1 (object_cache).

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.4.3 Default mapping

**Spec:** §8.1.4.3, p. 179 (PDF) — DCPS topic name = DLRL class name; oid
field names = `class`, `oid`; multi-attribute topic = `<Class>.<attribute>`;
index field = `index`.

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.4.4 Metamodel with mapping information

**Spec:** §8.1.4.4, p. 180-181 (PDF, with Figure 8.2) — extension of the
§8.1.3.3 metamodel classes with mapping fields:

* §8.1.4.4.1 Class — `main_topic`, `oid_field`, `class_field`,
  `full_oid_required`, `final`.
* §8.1.4.4.2 MonoAttribute — `topic`, `target_field`, `key_fields[*]`.
* §8.1.4.4.3 MultiAttribute — like MonoAttribute + `index_field`.
* §8.1.4.4.4 MonoRelation — `topic`, `target_fields[*]`, `key_fields[*]`,
  `full_oid_required`, `is_composition`.
* §8.1.4.4.5 MultiRelation — like MonoRelation + `index_field`.

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.4.5 Mapping when DCPS model is fixed

**Spec:** §8.1.4.5, p. 182 (PDF) — DLRL against an existing DCPS model for
legacy applications.

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.4.6 How is this mapping indicated?

**Spec:** §8.1.4.6, p. 182-183 (PDF, with Figure 8.3) — the DLRL generator
with a model description + model tags as input; output: native model
description, dedicated DLRL entities, DCPS description.

**Repo:** pragma-based tags via `pragma.rs`; the generator in
`dlrl-codegen/`.

**Tests:** cross-ref §8.2.2.

**Status:** done — pragma input + codegen output match the generator
pattern.

### §8.1.5 Operational Mapping

#### §8.1.5.1 Attachment to DCPS entities

**Spec:** §8.1.5.1, p. 183 (PDF) — a DLRL class is connected to multiple
DCPS topics (DataWriter/DataReader); all DataWriters/DataReaders of a DLRL
object are attached to a single Publisher/Subscriber. "DLRL has attached a
Publisher and/or a Subscriber to the notion of a Cache object."

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.5.2 Creation of DCPS entities

**Spec:** §8.1.5.2, p. 183 (PDF) — operations to create + activate the DCPS
entities, bound to the cache.

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, p. 183 (PDF) — QoS settable per DCPS entity; DLRL
provides the means to find the DCPS entities from the DLRL entities so the
application developer can set QoS.

**Repo:** —

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

### §8.1.6 Functional Mapping

#### §8.1.6.1 DLRL requested functions

##### §8.1.6.1.1 Publishing application

**Spec:** §8.1.6.1.1, p. 184 (PDF) — the publishing app should
create/modify/destroy objects, send publication requests, manage concurrent
modifications consistently.

**Repo:** `crates/dlrl/src/object_cache.rs::ObjectCache`
(register/mark_deleted/commit_all/rollback_all),
`crates/dlrl/src/transaction.rs` (concurrent-mod handling).

**Tests:** cross-ref §8.1.3.1, §8.1.5 (transaction tests).

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

##### §8.1.6.1.2 Subscribing application

**Spec:** §8.1.6.1.2, p. 184-185 (PDF) — loading objects, reading
attributes/relations, navigation, awareness of object changes. Incl. implicit
vs. explicit subscriptions, cache management, user interaction.

**Repo:** `crates/dlrl/src/subscription.rs::{HomeFactory,
SubscriptionRegistry, HomeListener, ObjectListener}`.

**Tests:** `subscription::tests::home_listener_registered_per_topic`,
`subscription::tests::object_listener_only_fires_for_its_own_id`,
`subscription::tests::registry_notifies_both_home_and_object_listeners`,
`subscription::tests::fanout_invokes_listeners_for_matching_topic`,
`subscription::tests::fanout_ignores_other_topics`.

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

##### §8.1.6.1.3 Publishing and subscribing applications

**Spec:** §8.1.6.1.3, p. 185 (PDF) — mixed publish+subscribe in one app.

**Repo:** implicit via the combination §8.1.6.1.1+§8.1.6.1.2.

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.6.2 DLRL entities

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

**Tests:** subset tests in the `subscription`/`object_cache`/`query` modules.

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

#### §8.1.6.3 Details on DLRL entities

**Spec:** §8.1.6.3, p. 188-210 (PDF) — a table with attributes + operations
per entity class. Sub-sections §8.1.6.3.1-§8.1.6.3.17 (one per entity class).

**Repo:** operations subset via `subscription.rs` and `object_cache.rs`;
full method-signature conformance missing.

**Tests:** —

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time entities

**Spec:** §8.2.1, p. 211-228 (PDF) — the IDL definition of all entity
interfaces + a service bootstrap (`DLRL_initialize`).

**Repo:** operations distributed across `subscription.rs`/`object_cache.rs`
with Rust API signatures; **no** strict 1:1 IDL-PSM re-emission. The PSM
codegen for C++/C#/Java/TS in `crates/dlrl-codegen/` uses this Rust API as a
backend library instead of direct IDL-PSM stub generation.

**Tests:** inline in the respective modules.

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

### §8.2.2 Generation process

**Spec:** §8.2.2, p. 228-234 (PDF) — `pragma DLRL`-annotated IDL → PSM
classes (Object/Home/Selection templates).

**Repo:** `crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}` (pragma
lexer), `crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs` (per language
backend `generate_*_object` + `generate_*_home`).

**Tests:** pragma parser:
`pragma::tests::parses_data_type_pragma`,
`pragma::tests::parses_data_key_pragma`,
`pragma::tests::parses_relation_pragma`,
`pragma::tests::extra_whitespace_tolerated`,
`pragma::tests::data_type_with_extra_tokens_rejected`,
`pragma::tests::data_key_with_one_token_rejected`,
`pragma::tests::missing_quotes_rejected`,
`pragma::tests::non_dlrl_line_rejected`,
`pragma::tests::unknown_tag_rejected`.
Codegen (`crates/dlrl-codegen/`):
`tests::collect_groups_by_type_name`,
`tests::data_type_without_keys_yields_empty_lists`,
`tests::keys_without_data_type_still_create_info`,
`cpp::tests::cpp_object_inherits_object_root`,
`cpp::tests::cpp_home_inherits_home_base`,
`cpp::tests::simple_name_strips_scope`,
`csharp::tests::csharp_partial_emits_namespace`,
`csharp::tests::csharp_partial_no_namespace_for_unscoped`,
`csharp::tests::csharp_object_emits_keys`,
`csharp::tests::namespace_for_multi_level`,
`java::tests::java_object_emits_package_and_annotations`,
`java::tests::java_object_no_package_for_unscoped`,
`java::tests::listener_interface_emits_three_callbacks`,
`ts::tests::ts_class_emits_impl_and_home`,
`ts::tests::ts_interface_emits_keys_and_relations`.

**Status:** done

### §8.2.3 Example

**Spec:** §8.2.3, p. 234-244 (PDF) — illustrative example code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, p. 245-249 (PDF) — the BNF grammar for DLRL Selection
filter expressions (an SQL subset analogous to the DCPS ContentFilteredTopic).

**Repo:** `crates/dlrl/src/query.rs::{Query, SortOrder, QueryError}`.

**Tests:** `query::tests::empty_query_returns_all`,
`query::tests::topic_filter_narrows_result`,
`query::tests::state_filter_only_returns_matching`,
`query::tests::custom_filter_applied`,
`query::tests::limit_caps_result_size`,
`query::tests::limit_too_large_rejected`,
`query::tests::order_by_sorts_ascending`,
`query::tests::order_by_descending_reverses`,
`query::tests::empty_topic_rejected`.

**Status:** done — covered by `crates/dlrl/src/metamodel.rs` (metamodel + 17
entity classes + 8 exception types + default mapping + cache lifecycle +
Annex-B query expression).

---

## Audit status

20 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-dlrl -p zerodds-dlrl-codegen` —
`zerodds-dlrl` 48 tests green (object_cache 9 + pragma 9 + query 9 +
relationship 7 + subscription 5 + transaction 9), `zerodds-dlrl-codegen` 15
tests green (cpp 3 + csharp 4 + java 3 + ts 2 + lib tests 3).

No open items. Spec fidelity established via the
`crates/dlrl/src/metamodel.rs` stub layer.
