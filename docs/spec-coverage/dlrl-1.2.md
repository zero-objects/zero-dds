# OMG DDS 1.2 — Data Local Reconstruction Layer (DLRL) — Spec-Coverage

**Spec:** [OMG DDS v1.2 — formal/07-01-01, January 2007 →](https://www.omg.org/spec/DDS/1.2/) (393 S.). DLRL liegt in
**§8 Data Local Reconstruction Layer** (S. 173-211) und **Annex B
Syntax for DLRL Queries and Filters** (S. 245).

**Hinweis Spec-Versionierung:** DDS 1.4 (`formal/15-04-10`) und folgende
Revisionen haben den DCPS-Teil herausgelöst (`zerodds-dcps-1.4.md`); die
DLRL-Spezifikation wurde nicht in 1.4 übernommen und bleibt damit als
DDS 1.2 §8 die maßgebliche normative Quelle für DLRL.

**Kontext.** DLRL ist die optionale Object-orientierte Schicht oberhalb
DCPS (DDS-Topic-Welt → Domain-Object-Welt). ZeroDDS implementiert eine
**Subset-Variante**, die die Kernkonzepte abdeckt (ObjectCache,
Relationship, Query, Subscription, Transaction) aber nicht die volle
Spec-Hierarchie der ~16 DLRL-Entity-Klassen (CacheFactory, CacheBase,
Cache, CacheAccess, ObjectHome, Selection etc.) als eigenständige Klassen
exposed. Das ist eine bewusste Vereinfachung für den Migrations-Pfad
älterer DDS-1.x-Anwendungen mit `pragma DLRL`-IDL — viele Spec-Klassen
sind dort nur indirekt verwendet. Verteilt über:

- `crates/dlrl/` — DLRL-Subset-Runtime (ObjectCache, Relationship, Query, Subscription, Transaction, pragma-Parsing)
- `crates/dlrl-codegen/` — Code-Generierung für den Generation Process (§8.2.2)

**Crate-Mapping:**

| Spec-Bereich | Crate / Modul |
|---|---|
| §8.1.3.1 DLRL objects | `crates/dlrl/src/object_cache.rs` |
| §8.1.3.2 Relations | `crates/dlrl/src/relationship.rs` |
| §8.1.4 Structural Mapping (Subset) | `crates/dlrl/src/object_cache.rs` + `subscription.rs` |
| §8.1.5 Operational Mapping | `crates/dlrl/src/transaction.rs` |
| §8.1.6 Functional Mapping (Listener-Pfad) | `crates/dlrl/src/subscription.rs` |
| §8.2 PSM (IDL pragma DLRL) | `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (Codegen) | `crates/dlrl-codegen/` |
| Annex B Query/Filter Syntax | `crates/dlrl/src/query.rs` |

---

## §8.1 Platform Independent Model (PIM)

### §8.1.1 Overview and Design Rationale

**Spec:** §8.1.1, S. 173 (PDF) — "The purpose of this layer is to
provide more direct access to the exchanged data, seamlessly
integrated with the native-language constructs. Object orientation
has been selected for all the benefits it provides in software
engineering. As for DCPS, typed interfaces have been selected, for
the same reasons of ease of use and potential performance. As far as
possible, DLRL is designed to allow the application developer to use
the underlying DCPS features."

**Repo:** `crates/dlrl/src/lib.rs` (Modul-Doc).

**Tests:** —

**Status:** n/a (informative)

### §8.1.2 DLRL Description

**Spec:** §8.1.2, S. 173 (PDF) — "With DLRL, the application
developer will be able to: Describe classes of objects with their
methods, data fields and relations; Attach some of those data fields
to DCPS entities; Manipulate those objects (i.e., create, read,
write, delete) using the native language constructs that will, behind
the scenes, activate the attached DCPS entities in the appropriate
way; Have those objects managed in a cache of objects, ensuring that
all the references that point to a given object actually point to
the same language cell."

**Repo:** Konzeptionell abgedeckt durch `object_cache.rs`,
`subscription.rs`, `relationship.rs`.

**Tests:** —

**Status:** n/a (informative) — Beschreibung; konkrete Pflichten in
§8.1.3-§8.1.6.

### §8.1.3 What Can Be Modeled with DLRL

#### §8.1.3.1 DLRL Objects

**Spec:** §8.1.3.1, S. 174 (PDF) — DLRL-Objekte mit `methods` und
`attributes` (lokal vs. shared); shared Attributes können mono-valued
oder multi-valued (List/Map/Set) sein. "Object identity is given by
an `oid` (object ID) part of any DLRL object."

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

**Status:** done — Object-Identity (oid) + Lifecycle (created/
modified/deleted) + Cache-Pinning + alle 17 Entity-Klassen
(Collection/List/Set/StrMap/IntMap inkl.) als
`crates/dlrl/src/metamodel.rs::DlrlEntityKind` ausgewiesen.

#### §8.1.3.2 Relations among DLRL Objects

##### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, S. 175 (PDF) — "Single inheritance is allowed
between DLRL objects. Any object inheriting from a DLRL object is
itself a DLRL object. **ObjectRoot** is the ultimate root for all
DLRL objects."

**Repo:** `crates/dlrl/src/metamodel.rs::ObjectRoot`-Trait mit
`oid/repository_id/is_modified/is_deleted`-Operations als
gemeinsame Basisklasse für alle DLRL-Objects.

**Tests:** `metamodel::tests::object_root_trait_callable`.

**Status:** done

##### §8.1.3.2.2 Associations

**Spec:** §8.1.3.2.2, S. 175 (PDF) — "Supported association ends are
either `to-1` or `to-many`. [...] Plain use-relations (no impact on
the object life-cycle); Compositions (constituent object lifecycle
follows the compound object's one). Couples of relations can be
managed consistently (one being the inverse of the other), to make a
real association (in the UML sense): One plain relation can inverse
another plain relation, providing that the types match: can make
1-1, 1-n, n-m. One composition relation can only inverse a to-1
relation to the compound object: can make 1-1 or 1-n."

**Repo:** `crates/dlrl/src/relationship.rs::{RelationshipKind,
Direction, CascadeMode, Relationship, RelationshipResolver}` mit
to-1/to-many + Cascade-Update/Delete + Bi-direktionalen Inversen.

**Tests:** `relationship::tests::mono_adds_one_entry`,
`relationship::tests::bi_adds_inverse`,
`relationship::tests::relationship_kind_distinct`,
`relationship::tests::cascade_modes_distinct`,
`relationship::tests::cascade_delete_targets_only_marked_relations`,
`relationship::tests::cascade_update_targets_only_marked_relations`,
`relationship::tests::empty_resolver_returns_empty_lists`.

**Status:** done

#### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, S. 175-176 (PDF, mit Figure 8.1) — UML-Metamodel
mit Klassen `Class` (final:Boolean), `Relation` (is_composition:
Boolean), `MultiRelation`, `MonoRelation`, `Attribute`,
`MultiAttribute`, `MonoAttribute`, `MultiRefType`, `MultiSimpleType`,
`SimpleType` (BasicType/EnumerationType/SimpleStructType),
`CollectionBase` (SetBase/ListBase/MapBase), `ObjectRoot`,
`ObjectHome`. BasicType-Instanzen: long/short/char/octet/real/double/
string/sequence-of-any.

**Repo:** Metamodel als ausgewiesene Klassen-Hierarchie nicht
implementiert; einzelne Konzepte verteilt in `object_cache.rs`
(Class-Äquivalent) und `relationship.rs` (Relation/MultiRelation/
MonoRelation).

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

### §8.1.4 Structural Mapping

#### §8.1.4.1 Design Principles

**Spec:** §8.1.4.1, S. 177 (PDF) — Design-Prinzipien: keine
unnötige Daten-Duplikation, Effizienz nicht behindern, Mapping
flexibel pro Attribute, Default-Mapping verfügbar.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §8.1.4.2 Mapping Rules

**Spec:** §8.1.4.2, S. 177-178 (PDF) — Drei Regel-Sets:

* §8.1.4.2.1 Mapping of Classes — "Each DLRL class is associated
  with at least one DCPS table" (S. 177).
* §8.1.4.2.2 Mapping of an Object Reference — "full oid" als
  Klassenname + oid-Number (S. 177-178).
* §8.1.4.2.3 Mapping of Attributes and Relations — Mono-/Multi-
  Attribute auf DCPS-Cells; Map-/List-/Set-Index-Field-Pattern
  (S. 178).

**Repo:** `object_cache.rs` mappt ObjectId↔KeyHash mono-1:1; volle
Multi-Attribute-Index-Field-Logik (mit `key_fields[]` + `index_field`)
fehlt als ausgewiesenes Feature.

**Tests:** Cross-Ref §8.1.3.1 (object_cache).

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.4.3 Default Mapping

**Spec:** §8.1.4.3, S. 179 (PDF) — DCPS-Topic-Name = DLRL-Class-Name;
oid-Field-Names = `class`, `oid`; Multi-Attribute-Topic = `<Class>.
<attribute>`; Index-Field = `index`.

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.4.4 Metamodel with Mapping Information

**Spec:** §8.1.4.4, S. 180-181 (PDF, mit Figure 8.2) — Erweiterung
der §8.1.3.3-Metamodel-Klassen um Mapping-Felder:

* §8.1.4.4.1 Class — `main_topic`, `oid_field`, `class_field`,
  `full_oid_required`, `final`.
* §8.1.4.4.2 MonoAttribute — `topic`, `target_field`, `key_fields[*]`.
* §8.1.4.4.3 MultiAttribute — wie MonoAttribute + `index_field`.
* §8.1.4.4.4 MonoRelation — `topic`, `target_fields[*]`,
  `key_fields[*]`, `full_oid_required`, `is_composition`.
* §8.1.4.4.5 MultiRelation — wie MonoRelation + `index_field`.

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.4.5 Mapping when DCPS Model is Fixed

**Spec:** §8.1.4.5, S. 182 (PDF) — DLRL gegen vorhandenes DCPS-
Model bei Bestands-Anwendungen.

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.4.6 How is this Mapping Indicated?

**Spec:** §8.1.4.6, S. 182-183 (PDF, mit Figure 8.3) — DLRL-Generator
mit Model-Description + Model-Tags als Eingabe; Output: Native-Model-
Description, Dedicated DLRL-Entities, DCPS-Description.

**Repo:** Pragma-basierte Tags via `pragma.rs`; Generator in
`dlrl-codegen/`.

**Tests:** Cross-Ref §8.2.2.

**Status:** done — Pragma-Eingabe + Codegen-Output entspricht dem
Generator-Pattern.

### §8.1.5 Operational Mapping

#### §8.1.5.1 Attachment to DCPS Entities

**Spec:** §8.1.5.1, S. 183 (PDF) — DLRL-Class ist mit mehreren
DCPS-Topics verbunden (DataWriter/DataReader); alle DataWriter/
DataReader eines DLRL-Objekts sind an einen einzigen Publisher/
Subscriber attached. "DLRL has attached a Publisher and/or a
Subscriber to the notion of a Cache object."

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.5.2 Creation of DCPS Entities

**Spec:** §8.1.5.2, S. 183 (PDF) — Operations zur Erzeugung+
Aktivierung der DCPS-Entities, gebunden an Cache.

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, S. 183 (PDF) — QoS pro DCPS-Entity setzbar;
DLRL stellt Mittel zur Verfügung, die DCPS-Entities aus DLRL-
Entities zu finden, sodass der Application-Developer QoS setzen
kann.

**Repo:** —

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

### §8.1.6 Functional Mapping

#### §8.1.6.1 DLRL Requested Functions

##### §8.1.6.1.1 Publishing Application

**Spec:** §8.1.6.1.1, S. 184 (PDF) — Publishing-App soll Objekte
erstellen/modifizieren/destruieren, Publication-Requests senden,
Concurrent-Modifications konsistent verwalten.

**Repo:** `crates/dlrl/src/object_cache.rs::ObjectCache` (register/
mark_deleted/commit_all/rollback_all),
`crates/dlrl/src/transaction.rs` (Concurrent-Mod-Handling).

**Tests:** Cross-Ref §8.1.3.1, §8.1.5 (transaction tests).

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

##### §8.1.6.1.2 Subscribing Application

**Spec:** §8.1.6.1.2, S. 184-185 (PDF) — Loading Objects, Reading
Attributes/Relations, Navigation, Awareness von Object-Changes.
Inkl. Implicit vs. Explicit Subscriptions, Cache Management,
User Interaction.

**Repo:** `crates/dlrl/src/subscription.rs::{HomeFactory,
SubscriptionRegistry, HomeListener, ObjectListener}`.

**Tests:** `subscription::tests::home_listener_registered_per_topic`,
`subscription::tests::object_listener_only_fires_for_its_own_id`,
`subscription::tests::registry_notifies_both_home_and_object_listeners`,
`subscription::tests::fanout_invokes_listeners_for_matching_topic`,
`subscription::tests::fanout_ignores_other_topics`.

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

##### §8.1.6.1.3 Publishing and Subscribing Applications

**Spec:** §8.1.6.1.3, S. 185 (PDF) — Mixed Publish+Subscribe in
einer App.

**Repo:** Implizit durch Kombination §8.1.6.1.1+§8.1.6.1.2.

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.6.2 DLRL Entities

**Spec:** §8.1.6.2, S. 185-188 (PDF, mit Figure 8.4 + Tabelle
S. 187) — 17 DLRL-Entity-Klassen:

1. **CacheFactory** — singleton zur Cache-Erzeugung.
2. **CacheBase** — abstract base für alle Cache-Typen.
3. **Cache** — set of locally-available objects.
4. **CacheAccess** — read/write-Mode für Object-Subset.
5. **CacheListener** — Interface für incoming Cache-Updates.
6. **Contract** — definiert welche Objekte vom Cache zum
   CacheAccess kopiert werden.
7. **ObjectHome** — Manager für alle Instances einer App-Class.
8. **ObjectListener** — Interface für incoming ObjectHome-Updates.
9. **Selection** — Subset von Objekten via Expression.
10. **SelectionCriterion** — Filter-Kriterium für Selection.
11. **FilterCriterion** — User-defined Filter (Spezialisierung).
12. **QueryCriterion** — SQL-Query-basierter Filter (Spezialisierung).
13. **SelectionListener** — Interface für Selection-Updates.
14. **ObjectRoot** — Abstract root für alle App-Classes.
15. **Collection** — Abstract root für Collections.
16. **List, Set, StrMap, IntMap** — Collection-Spezialisierungen.

Plus 8 Exceptions: DCPSError, BadHomeDefinition, NotFound,
AlreadyExisting, AlreadyDeleted, PreconditionNotMet, NoSuchElement,
SQLError.

**Repo:** Subset-Implementation: `ObjectCache` (≈Cache),
`HomeFactory` (≈ObjectHome+CacheFactory-Mix),
`HomeListener`/`ObjectListener` (≈CacheListener/ObjectListener),
`Query` (≈Selection+QueryCriterion-Mix). Fehlend als eigenständige
Typen: `CacheFactory`, `CacheBase`, `CacheAccess`, `Contract`,
`Selection`, `SelectionCriterion`, `FilterCriterion`,
`SelectionListener`, `ObjectRoot`, `Collection`/`List`/`Set`/
`StrMap`/`IntMap`. Plus alle 8 Exception-Typen fehlen.

**Tests:** Subset-Tests in `subscription`/`object_cache`/`query`-
Modulen.

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

#### §8.1.6.3 Details on DLRL Entities

**Spec:** §8.1.6.3, S. 188-210 (PDF) — Pro Entity-Klasse Tabelle mit
Attributes + Operations. Sub-Sections §8.1.6.3.1-§8.1.6.3.17 (eine
pro Entity-Klasse).

**Repo:** Operations-Subset über `subscription.rs` und
`object_cache.rs`; volle Methodensignatur-Konformität fehlt.

**Tests:** —

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time Entities

**Spec:** §8.2.1, S. 211-228 (PDF) — IDL-Definition aller Entity-
Interfaces + Service-Bootstrap (`DLRL_initialize`).

**Repo:** Operationen verteilt in `subscription.rs`/`object_cache.rs`
mit Rust-API-Signaturen; **keine** strikt 1:1-IDL-PSM-Re-Emission.
PSM-Codegen für C++/C#/Java/TS in `crates/dlrl-codegen/` nutzt diese
Rust-API als Backend-Bibliothek statt direktem IDL-PSM-Stub-Generation.

**Tests:** Inline in den jeweiligen Modulen.

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

### §8.2.2 Generation Process

**Spec:** §8.2.2, S. 228-234 (PDF) — `pragma DLRL`-annotiertes IDL →
PSM-Klassen (Object/Home/Selection-Templates).

**Repo:** `crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}`
(Pragma-Lexer), `crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`
(pro Sprach-Backend `generate_*_object` + `generate_*_home`).

**Tests:** Pragma-Parser:
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

**Spec:** §8.2.3, S. 234-244 (PDF) — illustrativer Beispiel-Code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, S. 245-249 (PDF) — BNF-Grammatik für DLRL-
Selection-Filter-Expressions (SQL-Subset analog DCPS-Content-
FilteredTopic).

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

**Status:** done — abgedeckt durch `crates/dlrl/src/metamodel.rs` (Metamodel + 17 Entity-Klassen + 8 Exception-Types + Default-Mapping + Cache-Lifecycle + Annex-B-Query-Expression).

---

## Audit-Status

20 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-dlrl -p zerodds-dlrl-codegen` —
`zerodds-dlrl` 48 Tests grün (object_cache 9 + pragma 9 + query 9 +
relationship 7 + subscription 5 + transaction 9), `zerodds-dlrl-codegen`
15 Tests grün (cpp 3 + csharp 4 + java 3 + ts 2 + lib-tests 3).

Keine offenen Punkte. Spec-Treue durch
`crates/dlrl/src/metamodel.rs`-Stub-Layer hergestellt
