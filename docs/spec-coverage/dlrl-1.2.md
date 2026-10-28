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
- `tools/idlc/src/dlrl.rs` — CLI-Front-End (`zerodds-idlc dlrl`), verdrahtet
  `dlrl-codegen` als Subcommand; siehe §8.2.2 zur Pragma-Namens-Kollision
  mit OpenDDS' `DCPS_DATA_TYPE`/`DCPS_DATA_KEY`-Dialekt

**Crate-Mapping:**

| Spec-Bereich | Crate / Modul |
|---|---|
| §8.1.3.1 DLRL objects | `crates/dlrl/src/object_cache.rs` |
| §8.1.3.2 Relations | `crates/dlrl/src/relationship.rs` |
| §8.1.4 Structural Mapping | `crates/dlrl/src/metamodel.rs` + `object_cache.rs` |
| §8.1.5 Operational Mapping | `crates/dlrl/src/transaction.rs` |
| §8.1.6 Functional Mapping (Listener-Pfad) | `crates/dlrl/src/subscription.rs` |
| §8.2 PSM (IDL pragma DLRL) | `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (Codegen) | `crates/dlrl-codegen/`, CLI: `tools/idlc/src/dlrl.rs` |
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

## §8.1.3 What Can Be Modeled with DLRL

### §8.1.3.1 DLRL Objects

**Spec:** §8.1.3.1, S. 174 (PDF) — DLRL-Objekte mit `methods` und
`attributes` (lokal vs. shared); shared Attributes können mono-valued
oder multi-valued (List/Map/Set) sein. "Object identity is given by
an `oid` (object ID) part of any DLRL object."

**Repo:** `crates/dlrl/src/object_cache.rs::{ObjectId, ObjectState,
ObjectRef, WeakObjectRef, ObjectCache}`.

**Tests:** `crates/dlrl/src/object_cache.rs::tests::{register_then_get_round_trip,
ids_returns_stable_order, mark_deleted_then_commit_removes,
mark_deleted_unknown_returns_false,
re_register_increments_version_and_marks_modified,
rollback_after_commit_restores_modified_to_committed,
rollback_drops_new_objects, weak_ref_invalidated_on_modify,
weak_ref_resolves_at_same_version}` (9).

**Status:** done — Object-Identity (oid) + Lifecycle (created/
modified/deleted) + Cache-Pinning.

### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, S. 175 (PDF) — "Single inheritance is allowed
between DLRL objects. Any object inheriting from a DLRL object is
itself a DLRL object. **ObjectRoot** is the ultimate root for all
DLRL objects."

**Repo:** `crates/dlrl/src/metamodel.rs::ObjectRoot`-Trait mit
`oid/repository_id/is_modified/is_deleted`-Operations als
gemeinsame Basisklasse für alle DLRL-Objects.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::object_root_trait_callable`.

**Status:** done

### §8.1.3.2.2 Associations

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

**Tests:** `crates/dlrl/src/relationship.rs::tests::{mono_adds_one_entry,
bi_adds_inverse, relationship_kind_distinct, cascade_modes_distinct,
cascade_delete_targets_only_marked_relations,
cascade_update_targets_only_marked_relations,
empty_resolver_returns_empty_lists}` (7).

**Status:** done

### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, S. 175-176 (PDF, mit Figure 8.1) — UML-Metamodel
mit Klassen `Class` (final:Boolean), `Relation` (is_composition:
Boolean), `MultiRelation`, `MonoRelation`, `Attribute`,
`MultiAttribute`, `MonoAttribute`, `MultiRefType`, `MultiSimpleType`,
`SimpleType` (BasicType/EnumerationType/SimpleStructType),
`CollectionBase` (SetBase/ListBase/MapBase), `ObjectRoot`,
`ObjectHome`. BasicType-Instanzen: long/short/char/octet/real/double/
string/sequence-of-any.

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel, ObjectRoot,
DlrlEntityKind}`; die Relation-Seite zusätzlich in
`crates/dlrl/src/relationship.rs::{RelationshipKind, Relationship}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{class_metamodel_default_fields,
multi_attribute_metamodel_construct, mono_relation_metamodel_construct,
object_root_trait_callable, dlrl_entity_kinds_count_17}`;
`crates/dlrl/src/relationship.rs::tests::relationship_kind_distinct`.

**Status:** done — Metamodel-Klassen (Class/MultiAttribute/MonoRelation/
ObjectRoot) mit den Feldern aus Figure 8.1 ausgewiesen.

## §8.1.4 Structural Mapping

### §8.1.4.1 Design Principles

**Spec:** §8.1.4.1, S. 177 (PDF) — Design-Prinzipien: keine
unnötige Daten-Duplikation, Effizienz nicht behindern, Mapping
flexibel pro Attribute, Default-Mapping verfügbar.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §8.1.4.2 Mapping Rules

**Spec:** §8.1.4.2, S. 177-178 (PDF) — Drei Regel-Sets:

* §8.1.4.2.1 Mapping of Classes — "Each DLRL class is associated
  with at least one DCPS table" (S. 177).
* §8.1.4.2.2 Mapping of an Object Reference — "full oid" als
  Klassenname + oid-Number (S. 177-178).
* §8.1.4.2.3 Mapping of Attributes and Relations — Mono-/Multi-
  Attribute auf DCPS-Cells; Map-/List-/Set-Index-Field-Pattern
  (S. 178).

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel}` trägt die drei
Regel-Sets: `main_topic` (Klasse → DCPS-Table), `class_field` +
`oid_field` + `full_oid_required` (Object-Reference als full oid),
`topic` + `target_field`/`target_fields` + `key_fields` +
`index_field` (Attribute und Relations auf DCPS-Cells);
`crates/dlrl/src/object_cache.rs::ObjectId` ist die Laufzeit-Seite
des oid.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::mapping_rules_bind_class_reference_and_attributes_per_spec_8_1_4_2`
— prüft alle drei Regel-Sets in einem Durchgang (Klasse trägt ein
Topic, full oid ist Klassen- plus oid-Feld, Multi-Attribute liegt in
einem eigenen Topic mit Index-Feld und den Key-Feldern des Besitzers).
Ergänzend `crates/dlrl/src/metamodel.rs::tests::multi_attribute_metamodel_construct`,
`crates/dlrl/src/metamodel.rs::tests::mono_relation_metamodel_construct`.

**Status:** done

### §8.1.4.3 Default Mapping

**Spec:** §8.1.4.3, S. 179 (PDF) — DCPS-Topic-Name = DLRL-Class-Name;
oid-Field-Names = `class`, `oid`; Multi-Attribute-Topic = `<Class>.
<attribute>`; Index-Field = `index`.

**Repo:** `crates/dlrl/src/metamodel.rs::default_mapping::{topic_name,
multi_topic, DEFAULT_OID_FIELD, DEFAULT_CLASS_FIELD, DEFAULT_INDEX_FIELD}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{default_mapping_topic_name_uses_class,
default_mapping_multi_topic_dot_separated,
default_mapping_constants_match_spec}` (3).

**Status:** done

### §8.1.4.4 Metamodel with Mapping Information

**Spec:** §8.1.4.4, S. 180-181 (PDF, mit Figure 8.2) — Erweiterung
der §8.1.3.3-Metamodel-Klassen um Mapping-Felder:

* §8.1.4.4.1 Class — `main_topic`, `oid_field`, `class_field`,
  `full_oid_required`, `final`.
* §8.1.4.4.2 MonoAttribute — `topic`, `target_field`, `key_fields[*]`.
* §8.1.4.4.3 MultiAttribute — wie MonoAttribute + `index_field`.
* §8.1.4.4.4 MonoRelation — `topic`, `target_fields[*]`,
  `key_fields[*]`, `full_oid_required`, `is_composition`.
* §8.1.4.4.5 MultiRelation — wie MonoRelation + `index_field`.

**Repo:** `crates/dlrl/src/metamodel.rs::{ClassMetamodel,
MultiAttributeMetamodel, MonoRelationMetamodel}` mit genau diesen
Mapping-Feldern; die Tag-Quelle dafür in
`crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{class_metamodel_default_fields,
multi_attribute_metamodel_construct, mono_relation_metamodel_construct}`;
`crates/dlrl/src/pragma.rs::tests::{parses_relation_pragma,
parses_data_key_pragma}`.

**Status:** done

### §8.1.4.5 Mapping when DCPS Model is Fixed

**Spec:** §8.1.4.5, S. 182 (PDF) — DLRL gegen vorhandenes DCPS-
Model bei Bestands-Anwendungen.

**Repo:** `crates/dlrl/src/metamodel.rs::ClassMetamodel` (`main_topic`,
`oid_field`, `class_field` frei belegbar statt Default-Konvention);
`crates/dlrl/src/pragma.rs::parse_pragma` liest die Fremd-Namen aus
`#pragma DLRL data_type`/`data_key`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::class_metamodel_over_a_fixed_dcps_model_keeps_the_foreign_names`;
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma}`.

**Status:** done

### §8.1.4.6 How is this Mapping Indicated?

**Spec:** §8.1.4.6, S. 182-183 (PDF, mit Figure 8.3) — DLRL-Generator
mit Model-Description + Model-Tags als Eingabe; Output: Native-Model-
Description, Dedicated DLRL-Entities, DCPS-Description.

**Repo:** Model-Tags als `#pragma DLRL`-Zeilen in
`crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}`; der Generator
in `crates/dlrl-codegen/src/lib.rs::collect_type_infos` (Model-Description aus den
Tags) und die vier Sprach-Backends
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`.

**Tests:** Tags als Generator-Eingabe:
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma, parses_relation_pragma}`; Generator-Ausgabe aus
diesen Tags:
`crates/dlrl-codegen/src/lib.rs::tests::{everything_about_one_type_lands_in_one_entry,
every_backend_carries_every_key_and_relation, every_home_names_its_object_class}`.

**Status:** done — Pragma-Eingabe + Codegen-Output entspricht dem
Generator-Pattern.

## §8.1.5 Operational Mapping

### §8.1.5.1 Attachment to DCPS Entities

**Spec:** §8.1.5.1, S. 183 (PDF) — DLRL-Class ist mit mehreren
DCPS-Topics verbunden (DataWriter/DataReader); alle DataWriter/
DataReader eines DLRL-Objekts sind an einen einzigen Publisher/
Subscriber attached. "DLRL has attached a Publisher and/or a
Subscriber to the notion of a Cache object."

**Repo:** `crates/dlrl/src/metamodel.rs::CacheAttachmentState`
(Attach-Zustand des Cache an Publisher/Subscriber);
`crates/dlrl/src/subscription.rs::SubscriptionRegistry` (Topic →
Listener-Zuordnung, also die Reader-Seite der Attachment-Beziehung).

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`;
`crates/dlrl/src/subscription.rs::tests::{home_listener_registered_per_topic,
fanout_invokes_listeners_for_matching_topic, fanout_ignores_other_topics}`.

**Status:** done — Cache-Attachment-Zustände + Topic-gebundene
Listener-Zuordnung ausgewiesen.

### §8.1.5.2 Creation of DCPS Entities

**Spec:** §8.1.5.2, S. 183 (PDF) — Operations zur Erzeugung+
Aktivierung der DCPS-Entities, gebunden an Cache.

**Repo:** `crates/dlrl/src/metamodel.rs::{CacheMode,
CacheAttachmentState}` (Erzeugungs- und Aktivierungs-Zustände);
`crates/dlrl/src/object_cache.rs::ObjectCache::{register, mark_deleted,
commit_all}` (Objekt-Lebenszyklus am Cache).

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_modes_distinct`,
`crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`,
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/object_cache.rs::tests::mark_deleted_then_commit_removes`.

**Status:** done — Cache-gebundener Lebenszyklus ausgewiesen.

### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, S. 183 (PDF) — QoS pro DCPS-Entity setzbar;
DLRL stellt Mittel zur Verfügung, die DCPS-Entities aus DLRL-
Entities zu finden, sodass der Application-Developer QoS setzen
kann.

**Repo:** `crates/dlrl/src/metamodel.rs::{CacheMode,
CacheAttachmentState}` — über den Attachment-Zustand findet die
Anwendung die DCPS-Entities, an denen sie QoS setzt; die QoS-Werte
selbst liegen in `crates/dcps/`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::cache_attachment_states_distinct`,
`crates/dlrl/src/metamodel.rs::tests::cache_modes_distinct`.

**Status:** done — Auffinden der DCPS-Entities über den
Attachment-Zustand ausgewiesen; QoS-Werte selbst sind DCPS-Sache.

## §8.1.6 Functional Mapping

### §8.1.6.1.1 DLRL Requested Functions — Publishing Application

**Spec:** §8.1.6.1.1, S. 184 (PDF) — Publishing-App soll Objekte
erstellen/modifizieren/destruieren, Publication-Requests senden,
Concurrent-Modifications konsistent verwalten.

**Repo:** `crates/dlrl/src/object_cache.rs::ObjectCache` (register/
mark_deleted/commit_all/rollback_all),
`crates/dlrl/src/transaction.rs` (Concurrent-Mod-Handling).

**Tests:** Erstellen/Modifizieren/Löschen:
`crates/dlrl/src/object_cache.rs::tests::{register_then_get_round_trip,
re_register_increments_version_and_marks_modified,
mark_deleted_then_commit_removes, rollback_drops_new_objects}`;
konsistente Behandlung nebenläufiger Änderungen:
`crates/dlrl/src/transaction.rs::tests::{commit_without_concurrent_modify_succeeds,
commit_detects_optimistic_conflict, rollback_resets_state,
double_commit_fails, commit_after_rollback_fails,
each_transaction_gets_unique_id}`.

**Status:** done — Publikations-Lebenszyklus + Konflikterkennung bei
nebenläufiger Änderung ausgewiesen.

### §8.1.6.1.2 DLRL Requested Functions — Subscribing Application

**Spec:** §8.1.6.1.2, S. 184-185 (PDF) — Loading Objects, Reading
Attributes/Relations, Navigation, Awareness von Object-Changes.
Inkl. Implicit vs. Explicit Subscriptions, Cache Management,
User Interaction.

**Repo:** `crates/dlrl/src/subscription.rs::{HomeFactory,
SubscriptionRegistry, HomeListener, ObjectListener}`.

**Tests:** `crates/dlrl/src/subscription.rs::tests::{home_listener_registered_per_topic,
object_listener_only_fires_for_its_own_id,
registry_notifies_both_home_and_object_listeners,
fanout_invokes_listeners_for_matching_topic, fanout_ignores_other_topics}`
(5).

**Status:** done — Laden, Navigieren und Änderungs-Benachrichtigung
über Home- und Object-Listener ausgewiesen.

### §8.1.6.1.3 DLRL Requested Functions — Publishing and Subscribing Applications

**Spec:** §8.1.6.1.3, S. 185 (PDF) — Mixed Publish+Subscribe in
einer App.

**Repo:** Implizit durch Kombination §8.1.6.1.1+§8.1.6.1.2.

**Tests:** `crates/dlrl/src/subscription.rs::tests::one_application_publishing_and_subscribing_sees_its_own_changes`;
`crates/dlrl/src/object_cache.rs::tests::re_register_increments_version_and_marks_modified`,
`crates/dlrl/src/object_cache.rs::tests::mark_deleted_then_commit_removes`.

**Status:** done — eine Anwendung, die publiziert und subskribiert,
sieht ihre eigenen Änderungen über denselben Cache.

### §8.1.6.2 DLRL Entities

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

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kinds_count_17,
dlrl_entity_kind_spec_names_match, dlrl_entity_kinds_distinct,
dlrl_exception_repository_ids_distinct,
dlrl_exception_repo_id_omg_namespace}` — die 17 Entity-Klassen und die
8 Exception-Typen sind namentlich und mit ihren Repository-IDs
ausgewiesen; die Subset-Typen selbst:
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`,
`crates/dlrl/src/query.rs::tests::topic_filter_narrows_result`.

**Status:** partial — namentlich vollständig (`DlrlEntityKind` mit 17
Werten, `DlrlException` mit 8), als eigenständige Typen aber nur das
Subset `ObjectCache`/`HomeFactory`/`HomeListener`/`ObjectListener`/
`Query`. Es fehlen `CacheFactory`, `CacheBase`, `CacheAccess`,
`Contract`, `Selection`, `SelectionCriterion`, `FilterCriterion`,
`SelectionListener`, `Collection`/`List`/`Set`/`StrMap`/`IntMap` als
Typen — siehe das folgende `open`-Item.

### §8.1.6.2 (Rest) DLRL-Entity-Klassen als eigenständige Typen

**Spec:** §8.1.6.2, S. 185-188 (PDF, Figure 8.4) — die 13 Entity-Klassen
`CacheFactory`, `CacheBase`, `CacheAccess`, `Contract`, `Selection`,
`SelectionCriterion`, `FilterCriterion`, `SelectionListener`,
`Collection`, `List`, `Set`, `StrMap`, `IntMap` als eigenständige
Klassen mit ihren Attributen.

**Repo:** —

**Tests:** —

**Status:** open

### §8.1.6.3 Details on DLRL Entities

**Spec:** §8.1.6.3, S. 188-210 (PDF) — Pro Entity-Klasse Tabelle mit
Attributes + Operations. Sub-Sections §8.1.6.3.1-§8.1.6.3.17 (eine
pro Entity-Klasse).

**Repo:** Operations-Subset über `crates/dlrl/src/subscription.rs`
(Listener-Callbacks), `crates/dlrl/src/object_cache.rs`
(Cache-Operations) und `crates/dlrl/src/query.rs` (Selection-artige
Abfrage); die Exception-Typen der Tabellen in
`crates/dlrl/src/metamodel.rs::DlrlException`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kinds_count_17,
dlrl_entity_kind_spec_names_match, dlrl_entity_kinds_distinct,
dlrl_exception_repository_ids_distinct,
dlrl_exception_repo_id_omg_namespace}`;
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`.

**Status:** partial — die Exception-Typen und die Entity-Namen sind
vollständig, die Operations nur als Subset. Die
Methodensignatur-Konformität der Tabellen §8.1.6.3.1-§8.1.6.3.17 ist
nicht hergestellt — siehe das folgende `open`-Item.

### §8.1.6.3 (Rest) Methodensignaturen der Entity-Tabellen

**Spec:** §8.1.6.3, S. 188-210 (PDF) — pro Entity-Klasse eine Tabelle
mit Attributes und Operations; §8.1.6.3.1-§8.1.6.3.17 geben die
Signaturen vor.

**Repo:** —

**Tests:** —

**Status:** open

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time Entities

**Spec:** §8.2.1, S. 211-228 (PDF) — IDL-Definition aller Entity-
Interfaces + Service-Bootstrap (`DLRL_initialize`).

**Repo:** Die Run-time-Entities existieren als Rust-API in
`crates/dlrl/src/object_cache.rs`, `crates/dlrl/src/subscription.rs`,
`crates/dlrl/src/query.rs` und `crates/dlrl/src/transaction.rs`; die
Repository-IDs der PSM-Typen in
`crates/dlrl/src/metamodel.rs::{DlrlEntityKind, DlrlException}`. Eine
1:1-Re-Emission der IDL-Interfaces aus §8.2.1 samt
`DLRL_initialize`-Bootstrap gibt es nicht.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::{dlrl_entity_kind_spec_names_match,
dlrl_exception_repo_id_omg_namespace}` — die PSM-Namen und der
OMG-Repository-ID-Namensraum stimmen mit der IDL-Definition überein;
die Laufzeit-Seite:
`crates/dlrl/src/object_cache.rs::tests::register_then_get_round_trip`,
`crates/dlrl/src/subscription.rs::tests::registry_notifies_both_home_and_object_listeners`,
`crates/dlrl/src/transaction.rs::tests::commit_path_no_conflict`.

**Status:** partial — Namen und Repository-IDs des PSM stimmen, die
Laufzeit-Operationen existieren als Rust-API. Die IDL-Interface-
Definitionen und der `DLRL_initialize`-Bootstrap werden nicht emittiert
— siehe das folgende `open`-Item.

### §8.2.1 (Rest) IDL-PSM-Emission + DLRL_initialize

**Spec:** §8.2.1, S. 211-228 (PDF) — die IDL-Definition aller
Entity-Interfaces sowie der Service-Bootstrap `DLRL_initialize`.

**Repo:** —

**Tests:** —

**Status:** open

### §8.2.2 Generation Process

**Spec:** §8.2.2, S. 228-234 (PDF) — `pragma DLRL`-annotiertes IDL →
PSM-Klassen (Object/Home/Selection-Templates).

**Repo:** `crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}`
(Pragma-Lexer), `crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`
(pro Sprach-Backend `generate_*_object` + `generate_*_home`). CLI-Front-End:
`tools/idlc/src/dlrl.rs` (Subcommand `zerodds-idlc dlrl --lang
{cpp,csharp,java,typescript}`) — scannt den rohen IDL-Text direkt über
`zerodds_dlrl::pragma::parse_pragma`, unabhängig vom AST-Facet-Pfad, den
`idl-lower` für `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` im OpenDDS-Dialekt liest
(`crates/idl-lower/src/lower.rs`, Diagnose benennt beide Lesarten).

**Tests:** Pragma-Parser:
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
die letzten fünf prüfen alle vier Sprach-Backends (`cpp`, `csharp`,
`java`, `ts`) in einem Durchgang. CLI: `tools/idlc/tests/dlrl.rs` (20
Tests — Generierung pro Sprache, Java-Aufteilung in Object-/Home-Datei,
Pragma-Kollision mit OpenDDS in einer Datei und je einzeln, defekte
Pragma als Diagnose, `#include`, `--lang all`, Build-Contract, Key-Typen
aus der IDL, Attribute, strukturierte und mehrwertige Attribute, Abbruch
bei Key oder Attribut ohne Abbildung);
`tools/idlc/tests/dlrl_compiles.rs` (8 Tests — die Ausgabe übersetzt mit
`c++`, `dotnet`, `javac` und `tsc` gegen Stubs der fehlenden
Laufzeittypen, je einmal allein und einmal zusammen mit der
`idlc generate`-Ausgabe derselben IDL und der Wire-Runtime aus
`runtime/`; auf dem Linux-Testhost alle acht ausgeführt); zusätzliche
CLI-Parser-Tests in `tools/idlc/src/main.rs`. Kollisions-Diagnose:
`crates/idl-lower/tests/dialects.rs` (4 Tests).

Jeder Member des Datentyps, der weder Key noch Relation ist, wird ein
Attribut der Objektklasse (§8.1.2.2, §8.1.4.3). Vorher erzeugte der
Generator nur Keys und Relationen.

- Mono-valued, mit Getter und Setter (§8.2.3.5 setzt sie über das
  Objekt, `t1->x(1000.0)`): Basistyp, Enumeration oder einfache Struktur.
  Einfach heißt: jeder Member ist Basistyp, Enumeration, einfache
  Struktur oder festes Array davon, also eine feste Zahl DCPS-Felder
  (§8.1.4.2.3, Fußnote 3 zu §8.1.2.2).
- Multi-valued, mit Getter ohne Setter (eine Sammlung ändert sich über
  ihr eigenes `add`/`put`/`remove`, §8.1.6.3.16–19): `sequence` und
  eindimensionales Array als List, `map<string, T>` als StrMap,
  `map<Ganzzahltyp, T>` als IntMap; die Elemente wie mono-valued.
- Enumeration und Struktur tragen den Namen, den `idlc generate` für
  dieselbe Sprache vergibt: C++ `::demo::Money`, Java
  `ZdTypes.demo.Money` (Enumeration als `int`), C# `global::demo.Money`,
  TypeScript `demo.Money`. Die C++-Datei bindet dann `<stem>.hpp` ein,
  die TypeScript-Datei importiert aus `./<stem>.js`.
- Keys nehmen Enumerationen und einfache Strukturen ebenfalls an.
- Ein Member ohne Abbildung bricht den Lauf mit Namen und Grund ab:
  Union, verschachtelte Sammlung, Struktur mit Sammlung,
  mehrdimensionales Array, Map mit anderem Key-Typ, breiter String.

Die Zuordnung zu DCPS-Tabellen (Haupttabelle, eigenes Topic
`<Klasse>.<Attribut>` mit `index` je mehrwertigem Attribut, §8.1.4.3)
leistet eine DLRL-Laufzeit; für C++, C#, Java und TypeScript liefert
ZeroDDS keine, der Generator erzeugt die Klassenform.

**Status:** done

### §8.2.3 Example

**Spec:** §8.2.3, S. 234-244 (PDF) — illustrativer Beispiel-Code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## Annex B

### Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, S. 245-249 (PDF) — BNF-Grammatik für DLRL-
Selection-Filter-Expressions (SQL-Subset analog DCPS-Content-
FilteredTopic).

**Repo:** `crates/dlrl/src/query.rs::{Query, SortOrder, QueryError}`.

**Tests:** `crates/dlrl/src/query.rs::tests::{empty_query_returns_all,
topic_filter_narrows_result, state_filter_only_returns_matching,
custom_filter_applied, limit_caps_result_size, limit_too_large_rejected,
order_by_sorts_ascending, order_by_descending_reverses,
empty_topic_rejected}` (9);
`crates/dlrl/src/metamodel.rs::tests::query_expression_construct`.

**Status:** done — Filter-, Sortier- und Limit-Semantik des
Query-Subsets ausgewiesen.

---

## Audit-Status

17 done / 3 partial / 3 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-dlrl -p zerodds-dlrl-codegen` —
`zerodds-dlrl` 66 Tests grün, `zerodds-dlrl-codegen` 6 Tests grün,
0 failed. CLI-Front-End: `cargo test -p zerodds-idlc --test dlrl` — 15
Tests grün; Pragma-Kollisions-Diagnose: `cargo test -p zerodds-idl-lower
--test dialects` — 59 Tests grün (davon 4 zur Kollision).

Offene Punkte und Teil-Umsetzungen: `dlrl-1.2.open.md`.
