# OMG DDS 1.2 — Data Local Reconstruction Layer (DLRL) — Spec-Coverage

**Spec:** [OMG DDS v1.2 — formal/07-01-01, January 2007 →](https://www.omg.org/spec/DDS/1.2/) (393 S.). DLRL liegt in
**§8 Data Local Reconstruction Layer** (S. 173-211) und **Annex B
Syntax for DLRL Queries and Filters** (S. 245).

**Hinweis Spec-Versionierung:** DDS 1.4 (`formal/15-04-10`) und folgende
Revisionen haben den DCPS-Teil herausgelöst (`zerodds-dcps-1.4.md`); die
DLRL-Spezifikation wurde nicht in 1.4 übernommen und bleibt damit als
DDS 1.2 §8 die maßgebliche normative Quelle für DLRL.

**Kontext.** DLRL ist die optionale objektorientierte Schicht oberhalb
DCPS (DDS-Topic-Welt → Domain-Object-Welt). Gebaut sind das Modell, das
strukturelle Mapping und die 19 Laufzeit-Entities der Tabelle auf S. 187
über `zerodds-dcps`, darunter `Selection` mit ihren Kriterien und die
Collection-Klassen, mit allen Operationen der Tabellen von §8.1.6.3, und
das IDL des PSM (§8.2.1) mit dem impliziten IDL je Anwendungsklasse.
Offen sind die generierten typisierten Klassen (§8.2.2). `ObjectCache` und
`Transaction` sind eine ältere Teilmenge ohne Gegenstück in §8.
Verteilt über:

- `crates/dlrl/` — DLRL-Runtime (Modell und Typprüfung, strukturelles Mapping mit Zeilen; Laufzeit-Entities über DCPS; ObjectCache, Transaction, Annex-B-Ausdrücke, pragma-Parsing)
- `crates/dlrl-codegen/` — Code-Generierung für den Generation Process (§8.2.2)
- `tools/idlc/src/dlrl.rs` — CLI-Front-End (`zerodds-idlc dlrl`), verdrahtet
  `dlrl-codegen` als Subcommand; siehe §8.2.2 zur Pragma-Namens-Kollision
  mit OpenDDS' `DCPS_DATA_TYPE`/`DCPS_DATA_KEY`-Dialekt

**Crate-Mapping:**

| Spec-Bereich | Crate / Modul |
|---|---|
| §8.1.3 Modellierbares (Modell, Typprüfung) | `crates/dlrl/src/model.rs`, `crates/dlrl/src/types.rs` |
| §8.1.4 Structural Mapping | `crates/dlrl/src/mapping.rs` (Abbildung, Default-Mapping, Beschreibung der Topics), `crates/dlrl/src/rows.rs` (Zeilen) |
| §8.1.5 Operational Mapping | `crates/dlrl/src/cache/mod.rs` (DCPS-Entities eines Caches), `crates/dlrl/src/dcps_binding.rs` (DCPS-Typen, Zeile ↔ Sample) |
| §8.1.6 Functional Mapping | `crates/dlrl/src/cache/` (Cache, CacheAccess, ObjectHome, ObjectRoot, Listener; `selection.rs` Selection und Kriterien; `collection.rs` List, Set, StrMap, IntMap), PSM-Werttypen in `crates/dlrl/src/types.rs` |
| §8.2.1 IDL-PSM | `crates/dlrl/idl/` (generisches IDL), `crates/dlrl-codegen/src/idl.rs` (implizites IDL), CLI `tools/idlc/src/dlrl.rs` (`--lang idl`) |
| §8.2.2 Model Tags und Generator | `crates/dlrl/src/model_tags.rs`, `crates/dlrl/src/generation.rs`; Pragma-Dialekt `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (Codegen) | `crates/dlrl-codegen/`, CLI: `tools/idlc/src/dlrl.rs` |
| Annex B Query/Filter Syntax | `crates/dlrl/src/criterion.rs`, Grammatik `Dlrl12` in `crates/sql-filter/src/parser.rs` |

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

**Status:** n/a (informative) — Zweck und Entwurfsbegründung.

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

**Repo:** — (die Pflichten stehen in den Items zu §8.1.3 bis §8.1.6).

**Tests:** —

**Status:** n/a (informative) — Beschreibung; konkrete Pflichten in
§8.1.3-§8.1.6.

## §8.1.3 What Can Be Modeled with DLRL

### §8.1.3.1 DLRL Objects

**Spec:** §8.1.3.1, S. 174-175 (PDF) — Attribute sind lokal (nicht
verteilt) oder shared. "A DLRL object has at least one shared
attribute." Shared Attribute sind mono-valued (Basistyp, Aufzählung,
einfache Struktur, Referenz auf ein DLRL-Objekt) oder multi-valued
(Liste, Map, Set mit Elementen eines Typs). Typprüfung: strikte
Gleichheit für einfache Typen, Einschluss für Referenzen (eine
abgeleitete Klasse passt in eine Referenz oder Sammlung ihrer Basis).
"two different references to the same object [...] will actually point
to the same memory location." "Object identity is given by an oid
(object ID) part of any DLRL object."

**Repo:** `crates/dlrl/src/model.rs::{ClassModel, Attribute,
AttributeType, SimpleType, CollectionBasis, Model::new,
Model::check_value, ObjectRef}` (lokale Attribute in
`ClassModel::locals`, Regel „mindestens ein shared Attribut“ in
`Model::new`, Typprüfung je Wert und je Sammlungselement in
`Model::check_value`, eine Referenz auf `DlrlOid::NIL` weist es mit
`TypeError::NilOid` ab); `crates/dlrl/src/types.rs::DlrlOid`;
`crates/dlrl/src/rows.rs::ObjectData` (oid als Teil jedes Objekts und
jedes Schlüssels). Die oid, die ein Cache vergibt
(`crates/dlrl/src/cache/mod.rs::oid_creator`,
`crates/dlrl/src/rows.rs::KeyAssociation::process_wide`): erster Wert die
Host-Id der GUID-Präfixe, zweiter die Prozess-Id mit der Markierung ihres
PID-Namensraums, dritter ein Startwert aus dem Zufall des Betriebssystems
plus ein Zähler, den alle Caches des Prozesses teilen. Zwei Caches eines
Prozesses vergeben damit nie dieselbe oid, auch nicht auf zwei
Participants.

**Tests:** `crates/dlrl/tests/entities.rs::dlrl_8_1_3_1_two_caches_never_allocate_one_oid`;
`crates/dlrl/tests/objects.rs::{dlrl_8_1_3_1_simple_types_need_the_exact_type,
dlrl_8_1_3_1_collections_are_homogeneous_and_of_their_basis,
dlrl_8_1_3_1_a_derived_object_fits_a_reference_and_a_collection_of_its_base,
dlrl_8_1_3_1_a_class_without_shared_attribute_is_refused,
dlrl_8_1_3_1_a_local_attribute_is_not_published,
dlrl_8_1_3_1_the_oid_is_part_of_every_object,
dlrl_8_1_3_1_a_reference_to_the_nil_oid_is_refused}`; grün auf dem
Linux-Testhost.

**Status:** partial — Modell, lokale und shared Attribute, Typprüfung
und oid sind gebaut. Offen: eine Speicherzelle je Identität (Cache mit
`ObjectRoot` als geteiltem Handle, Item §8.1.6.2 Rest) und der Aufruf
von `Model::check_value` aus den `set_<attribute>`-Operationen der
erzeugten Klassen (Item §8.1.6.3 Rest).

### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, S. 175 (PDF) — "Single inheritance is allowed
between DLRL objects. Any object inheriting from a DLRL object is
itself a DLRL object. ObjectRoot is the ultimate root for all DLRL
objects. DLRL objects can, in addition, inherit from any number of
native language objects."

**Repo:** `crates/dlrl/src/model.rs::{ClassModel::bases, Model::new,
Model::parent, Model::children, Model::lineage, Model::is_kind_of,
Model::native_bases, OBJECT_ROOT_NAMES}`: genau eine DLRL-Basis
(`ObjectRoot` in den Schreibweisen `ObjectRoot`, `DDS::ObjectRoot`,
`DLRL::ObjectRoot` oder eine Klasse des Modells), beliebig viele native
Basen; eine `final`-Klasse wird nicht erweitert, Zyklen werden
abgewiesen.

**Tests:** `crates/dlrl/tests/relations.rs::{dlrl_8_1_3_2_1_track3d_is_a_track_and_an_object_root,
dlrl_8_1_3_2_1_two_dlrl_bases_are_refused,
dlrl_8_1_3_2_1_every_class_descends_from_object_root_and_no_final_class_is_extended}`;
grün auf dem Linux-Testhost.

**Status:** partial — die Vererbung im Modell ist gebaut. Offen: die
erzeugten Klassen (Track3D erweitert die erzeugte Klasse Track, Item
§8.2.2) und `ObjectHome` mit `parent` und `children` (Item §8.1.6.2
Rest).

### §8.1.3.2.2 Associations

**Spec:** §8.1.3.2.2, S. 175 (PDF) — "Supported association ends are
either to-1 or to-many. [...] Plain use-relations (no impact on the
object life-cycle); Compositions (constituent object lifecycle follows
the compound object's one). [...] One plain relation can inverse
another plain relation, providing that the types match: can make 1-1,
1-n, n-m. One composition relation can only inverse a to-1 relation to
the compound object: can make 1-1 or 1-n."

**Repo:** `crates/dlrl/src/model.rs::{Relation, Multiplicity,
Model::new}` (to-1 und to-many auf jeder Sammlungsbasis,
`is_composition`, `inverse`; `Model::new` prüft, dass beide Enden
einander nennen, dass jedes Ende auf die Besitzerklasse des anderen
zielt und dass die Inverse einer Komposition eine einfache
to-1-Relation ist); `crates/dlrl/src/mapping.rs::{MonoRelationMapping,
MultiRelationMapping}` tragen `is_composition`.

**Tests:** `crates/dlrl/tests/relations.rs::{dlrl_8_1_3_2_2_plain_inverses_make_1_1_1_n_and_n_m,
dlrl_8_1_3_2_2_inverse_relations_name_each_other_and_their_types_match,
dlrl_8_1_3_2_2_the_inverse_of_a_composition_must_be_to_1}`; grün auf
dem Linux-Testhost.

**Status:** partial — Relationen, Kompositionen und die beiden Regeln
für Inverse sind im Modell gebaut. Offen zur Laufzeit: das Setzen eines
Endes aktualisiert das andere (1-1, 1-n, n-m) und `destroy` eines
Ganzen zerstört seine Teile (Item §8.1.6.3 Rest, `ObjectRoot` und die
Sammlungen).

### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, S. 175-176 (PDF, Figure 8.1) — `Class` (`final`,
`parent`, `children`), `Attribute` mit `MonoAttribute` und
`MultiAttribute`, `Relation` (`is_composition`, `inverse`) mit
`MonoRelation` und `MultiRelation`, `SimpleType` (`BasicType`,
`EnumerationType`, `SimpleStructType`), `MultiSimpleType`,
`MultiRefType`, `CollectionBase` (`SetBase`, `ListBase`, `MapBase`).
BasicType: long, short, char, octet, real, double, string und Sequenzen
davon. "This metamodel is given for explanation purpose. This
specification does not require that it is implemented as such."

**Repo:** `crates/dlrl/src/model.rs` (die Zuordnung Figure 8.1 →
Rust-Typen steht in der Modul-Dokumentation): `ClassModel`,
`Attribute`/`AttributeType::{Mono, Multi}`,
`Relation`/`Multiplicity::{ToOne, ToMany}`, `SimpleType`, `BasicType`,
`BasicItem`, `EnumerationType`, `SimpleStructType`, `CollectionBasis`
(die Map-Basis in den beiden PSM-Formen `StrMap` und `IntMap`).

**Tests:** `crates/dlrl/tests/model.rs::{dlrl_8_1_3_3_every_construct_of_figure_8_1_is_describable,
dlrl_8_1_3_3_ill_formed_types_are_refused}`; grün auf dem
Linux-Testhost.

**Status:** done — jedes Konstrukt der Figure 8.1 ist beschreibbar und
erreicht unter dem Default-Mapping eine DCPS-Beschreibung.

## §8.1.4 Structural Mapping

### §8.1.4.1 Design Principles

**Spec:** §8.1.4.1, S. 177 (PDF) — Design-Prinzipien: keine
unnötige Daten-Duplikation, Effizienz nicht behindern, Mapping
flexibel pro Attribute, Default-Mapping verfügbar.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Entwurfsprinzipien; das Default-Mapping und die Abbildung je Attribut stehen in den folgenden Items zu §8.1.4.

### §8.1.4.2 Mapping Rules

**Spec:** §8.1.4.2, S. 177-178 (PDF) — DCPS-Daten als Tabellen: Topic
= Tabelle, Instanz = Zeile mit Schlüssel, Feld = Spalte.
§8.1.4.2.1: jede Klasse hat eine Haupttabelle; "A DLRL object is
considered to exist if it has a corresponding row in this table."
§8.1.4.2.2: eine Referenz ist Klassenname plus oid (full oid); der
Klassenname ist nötig, wenn mehrere Klassen eine Haupttabelle teilen
oder das Ziel einer Relation abgeleitete Klassen hat. §8.1.4.2.3:
mono-valued Attribute und Relationen als Zellen einer Zeile mit dem
Schlüssel des Objekts (Basistyp eine Zelle, Aufzählung eine Zelle
Integer oder String, Struktur so viele Zellen wie nötig, Referenz oid
oder full oid); multi-valued als eine Zeile je Element mit dem
Schlüssel des Besitzers plus Index (Integer für eine Liste, String
oder Integer für eine Map).

**Repo:** `crates/dlrl/src/rows.rs::{Mapping::rows_of,
Mapping::reconstruct, Row, Cell, ObjectData, Reconstruction,
SkipReason}`: `rows_of` schreibt die Zeilen eines Objekts,
`reconstruct` baut Objekte nur aus Zeilen der Haupttabelle und meldet
Zeilen ohne Hauptzeile, mit unbekannter oder hier nicht abgebildeter
Klasse in `Reconstruction::skipped`. Ein Set hat keinen Index, sein
Element ergänzt den Schlüssel. Eine Referenz auf kein Objekt ist ein
leerer Klassenteil mit `DlrlOid::NIL`; die nil-oid wird abgewiesen als
oid eines Objekts (`RowError::NilOid` in `rows_of`), in einer Referenz
auf ein Objekt (`TypeError::NilOid`), im Besitzerschlüssel einer
gelesenen Zeile (`RowError::NilOid` in `reconstruct`) und neben einem
Klassenteil (`RowError::BadCell`). Die Zeilen eines `NoOid`-Schlüssels
beschreibt das Item §8.1.4.5.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_2_1_an_object_exists_with_its_row_in_the_main_topic,
dlrl_8_1_4_2_2_the_class_part_is_required_for_a_shared_main_topic_and_for_a_target_with_derived_classes,
dlrl_8_1_4_2_2_the_nil_oid_designates_no_object,
dlrl_8_1_4_2_3_cells_per_attribute_kind,
dlrl_8_1_4_2_3_a_collection_is_one_row_per_item_keyed_by_owner_and_index,
dlrl_8_1_4_5_objects_over_a_keyed_topic_without_oid_fields}`; grün auf
dem Linux-Testhost.

**Status:** done — die drei Regelgruppen sind an den erzeugten und
gelesenen Zeilen geprüft.

### §8.1.4.3 Default Mapping

**Spec:** §8.1.4.3, S. 179 (PDF) — Haupttabelle: Topic-Name =
Klassenname, oid-Felder `class` und `oid`; mono-valued Attribute in der
Haupttabelle, Feldname = Attributname oder Attributname und Unterfeld
mit '.'; je multi-valued Attribut ein Topic `<Klasse>.<Attribut>` mit
dem zusätzlichen Schlüsselfeld `index`; abgeleitete Klassen mit einer
Erweiterungstabelle für die hinzugefügten mono-valued Attribute.
"This default mapping is overwritten by any mapping information
provided by the application developer."

**Repo:** `crates/dlrl/src/mapping.rs::{Mapping::new,
Mapping::default_for, default_mapping}`. Jeder nicht angegebene Teil
(Haupttopic, Erweiterungstopic, jedes Attribut) erhält den Default: eine
abgeleitete Klasse ohne eigenes Haupttopic liegt im Haupttopic ihrer
Basis und erhält das Erweiterungstopic `<Klasse>`, wenn sie eigene
mono-valued Member hat; der Schlüssel des Haupttopics ist
(`class`, `oid`), wenn mehrere Klassen es teilen, sonst (`oid`);
Referenzfelder heißen `<Attribut>` bzw. `<Attribut>.class` und
`<Attribut>.oid`. Eine Angabe ersetzt den Default nur für ihren Teil.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_3_the_minimal_tags_of_8_2_3_3_yield_the_default_topics_and_fields,
dlrl_8_1_4_3_a_given_mapping_overrides_the_default_per_attribute}`;
grün auf dem Linux-Testhost.

**Status:** done — nicht geprüft: ob DCPS-Implementierungen '.' in
Topic- und Feldnamen annehmen (die Default-Namen der Spezifikation).

### §8.1.4.4 Metamodel with Mapping Information

**Spec:** §8.1.4.4, S. 180-181 (PDF, Figure 8.2) — Class:
`main_topic`, `oid_field`, `class_field`, `full_oid_required`, `final`;
MonoAttribute: `topic`, `target_field`, `key_fields`; MultiAttribute
zusätzlich `index_field`, sein Topic "cannot be the same as the owner
Class::topic"; MonoRelation: `topic`, `target_fields` (1 oder 2 nach
`full_oid_required`), `key_fields`, `full_oid_required`,
`is_composition`; MultiRelation zusätzlich `index_field`. `key_fields`:
1 oder 2 nach der Klassendefinition.

**Repo:** `crates/dlrl/src/mapping.rs::{ClassMapping, MemberMapping,
MonoAttributeMapping, MultiAttributeMapping, MonoRelationMapping,
MultiRelationMapping, KeyDescription, KeyContent, Topic,
TopicDescription, ClassMappingSpec, MemberMappingSpec, TopicSpec,
MultiTopicSpec, MappingError}`; die Zugriffe `full_oid_required`,
`oid_field`, `class_field`, `key_fields`, `target_fields`,
`index_field`. `Mapping::new` prüft: Schlüsselfelder je Inhalt
(FullOid 2, SimpleOid 1, NoOid mindestens 1), Platz- und Sammlungstopics
bezeichnen den Besitzer wie das Haupttopic, ein Sammlungstopic ist
weder Topic der Besitzerklasse noch eines anderen Members, Set ohne
Indexfeld, je Zelle des Typs ein eigenes Wertfeld (ein wiederholter
Name ist `MappingError::RepeatedValueField`), ein Indexfeld oder ein
Elementfeld eines Sets wiederholt kein Feld des Besitzerschlüssels
(`MappingError::FieldConflict`), Zielfelder passend zum Haupttopic des
Ziels, Klassenteil wo mehrere Klassen Zeilen in einem
Topic haben, ein Topic mit einem Schlüssel, einem Typnamen und einem
Typ je Feld. Die Tag-Quelle (XML nach §8.2.2.3) folgt mit dem Item
§8.2.2.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_4_each_element_of_the_example_yields_its_topic_and_fields,
dlrl_8_1_4_4_a_collection_topic_differs_from_the_owner_topics,
dlrl_8_1_4_4_key_fields_follow_the_key_content,
dlrl_8_1_4_4_the_given_elements_fit_the_members}`; grün auf dem
Linux-Testhost.

**Status:** done — das Beispiel von §8.2.3.3 ergibt die fünf Topics von
§8.2.3.4 und das Haupttopic `RADAR-TOPIC` von Radar, das die Tabelle
von §8.2.3.4 nicht aufführt. Die Spezifikation schreibt `Y_TOPIC` und
`RADAR_OID` (XML) bzw. `Y-TOPIC` und `RADAR-OID` (Tabelle); der Test
nimmt die XML-Namen.

### §8.1.4.5 Mapping when DCPS Model is Fixed

**Spec:** §8.1.4.5, S. 182 (PDF) — ein vorhandenes DCPS-Modell wird auf
DLRL abgebildet; Teilnehmer auf DCPS- und auf DLRL-Ebene arbeiten in
einem System. Die Topic-Schlüssel identifizieren die Objekte; die
Implementierung verwaltet die Zuordnung Schlüssel ↔ oid.

**Repo:** `crates/dlrl/src/mapping.rs::{KeyContent::NoOid,
ClassMapping::identity, Mapping::identity_cells,
MappingError::NoOidKeyField}`: jedes Schlüsselfeld des Haupt-Topics
einer `NoOid`-Klasse ist das Wertfeld eines einwertigen Attributs eines
Basis- oder Aufzählungstyps; die Schlüsselfelder der übrigen Topics der
Klasse und die Zielfelder einer Relation auf sie nehmen dessen Typ.
`crates/dlrl/src/rows.rs::{KeyAssociation, Mapping::identity_key,
Mapping::rows_of_with, Mapping::reconstruct_with,
RowError::UnknownKey, RowError::NoObjectInNoOidReference}`: die
Zuordnung Schlüsselwerte ↔ oid; ein Schlüsselwert erhält beim ersten
Lesen eine lokale oid und behält sie.
`crates/dlrl/src/cache/home.rs::ObjectHome::{create_unregistered_object,
register_object}` leitet die oid aus den Schlüsselwerten ab
(`AlreadyExisting` für einen vergebenen Schlüssel);
`crates/dlrl/src/cache/object.rs::ObjectRoot::set` weist die Änderung
eines Schlüsselfelds eines registrierten Objekts ab (§8.1.6.3.14).
Ein Schlüssel wird wieder frei, wenn sein Objekt nicht mehr besteht:
`crates/dlrl/src/cache/mod.rs::Cache::{clean, publish,
release_unheld_key}` (Dispose über den DataReader des Caches; ein
zerstörtes und geschriebenes Objekt, das der Cache nicht hält, etwa in
einem WRITE_ONLY-Cache) und
`crates/dlrl/src/cache/access.rs::CacheAccess::{refresh, purge}` (ein
registriertes, nie geschriebenes Objekt, das `refresh`, `purge` oder
`delete_access` verwirft).
Fremdnamen für Topics, Typen und Felder: `TopicSpec`,
`crates/dlrl/src/pragma.rs::parse_pragma`.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_3_a_given_mapping_overrides_the_default_per_attribute,
dlrl_8_1_4_5_objects_over_a_keyed_topic_without_oid_fields,
dlrl_8_1_4_5_a_key_field_must_be_an_identity_attribute}`;
`crates/dlrl/tests/fixed_dcps_model.rs::{dlrl_8_1_4_5_a_fixed_dcps_model_is_read_and_written_as_objects,
dlrl_8_1_4_5_a_key_is_free_again_once_its_object_is_gone}`
(ein DCPS-Teilnehmer mit dem Typ `Legacy::StationType` aus IDL-Text,
ohne Klassen- und oid-Feld: Samples werden Objekte, ein Dispose löscht,
ein registriertes Objekt wird ein Sample; in einem WRITE_ONLY-Cache ist
der Schlüssel nach Zerstören und Schreiben, nach `refresh`, `purge` und
`delete_access` wieder frei, nach Schreiben ohne Zerstören nicht);
`crates/dlrl/tests/cyclone_fixed_model.rs::dlrl_8_1_4_5_a_fixed_cyclone_model_is_read_and_written_as_objects`
(ein C-Programm von Cyclone DDS, `crates/dlrl/tests/cyclone/station.c`,
auf demselben Topic in beide Richtungen; `#[ignore]`, Job
`vendor-live-dcps`);
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma}`; grün auf dem Linux-Testhost, der Cyclone-Test
mit Cyclone DDS 0.10.5.

**Status:** partial — gebaut und gegen Cyclone DDS 0.10.5 gemessen: Objekte
über ein festes DCPS-Modell in beide Richtungen und die Verwaltung der
Zuordnung Schlüssel ↔ oid. Nicht gemessen: gegen OpenSplice (nicht auf
dem Linux-Testhost) und gegen Cyclone DDS 11.0.1 des Images im Job
`vendor-live-dcps`. Offen: Vererbung zwischen Anwendungsklassen mit
`NoOid`-Schlüssel. Eine abgeleitete Klasse im Haupt-Topic ihrer
Basisklasse wird abgewiesen (`MappingError::ClassPartRequired`), weil
ohne Klassenfeld die Klasse einer Zeile nicht im Topic steht; §8.1.4.5
nennt diese Vererbung schwierig, aber machbar, und das Mapping
unterstützt. Einschränkungen dieser Implementierung, die die
Spezifikation nicht nennt: `ObjectHome::create_object` einer
`NoOid`-Klasse ergibt `PreconditionNotMet`, weil ein Objekt von
`create_object` registriert ist und die Identitätsfelder eines
registrierten Objekts nach §8.1.6.3.7 nicht mehr geändert werden dürfen;
es entsteht über `create_unregistered_object` und `register_object`.
Eine einwertige Relation auf eine `NoOid`-Klasse, die kein Objekt
bezeichnet, wird nicht geschrieben
(`RowError::NoObjectInNoOidReference`), weil die Schlüsselfelder des
Ziels keinen Wert für „kein Objekt“ haben.

### §8.1.4.6 How is this Mapping Indicated?

**Spec:** §8.1.4.6, S. 182-183 (PDF, mit Figure 8.3) — zwei orthogonale
Beschreibungen (Objektmodell mit shared-Teil, Mapping); der Generator
erzeugt die native Modellbeschreibung, die DLRL-Entities und auf Wunsch
die DCPS-Beschreibung.

**Repo:** Objektmodell `crates/dlrl/src/model.rs::Model` und Mapping
`crates/dlrl/src/mapping.rs::ClassMappingSpec` als getrennte Eingaben;
die DCPS-Beschreibung auf Wunsch:
`crates/dlrl/src/dcps_binding.rs::{dcps_description, topic_type}`; die
generischen DLRL-Entities in `crates/dlrl/src/cache/`. Abweichung vom
PSM: die DCPS-Beschreibung gibt `DDS::DLRLOid` als
`@final struct DLRLOid { long value0; long value1; long value2; }` aus,
das PSM-IDL (§8.2.1) als `struct DLRLOid { DLRL_OID_TYPE_NATIVE
value[3]; }`. Die CDR-Bytes sind gleich, der Typ (Membernamen, Struktur,
damit das TypeObject) nicht; ein oid-Feld einer anderen
DLRL-Implementierung in Array-Form ist nach XTypes nicht zuweisbar.
Grund laut `crates/dlrl/src/dcps_binding.rs`: ein oid-Feld ist ein
Schlüsselfeld, und der Key-Holder von `zerodds-wire-codec` nimmt kein
Array als Schlüsselmember an (hier nicht erneut gemessen). Der Generator
aus IDL mit Model-Tags: `crates/dlrl/src/pragma.rs::parse_pragma`,
`crates/dlrl-codegen/src/lib.rs::collect_type_infos` und die Backends
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`.

**Tests:** `crates/dlrl/tests/cache.rs::dlrl_8_1_4_6_the_dcps_type_of_each_topic_carries_its_rows`;
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma, parses_relation_pragma}`;
`crates/dlrl-codegen/src/lib.rs::tests::{everything_about_one_type_lands_in_one_entry,
every_backend_carries_every_key_and_relation, every_home_names_its_object_class}`;
grün auf dem Linux-Testhost.

**Status:** partial — die zwei Eingaben und die DCPS-Beschreibung sind
gebaut. Offen: der Generator, der aus Model-Description und Model-Tags
die native Modellbeschreibung und die typisierten DLRL-Entities über
`crates/dlrl/src/cache/` erzeugt (Item §8.2.2); `DDS::DLRLOid` in der
Array-Form des PSM, sobald `zerodds-wire-codec` ein Array als
Schlüsselmember annimmt.

## §8.1.5 Operational Mapping

### §8.1.5.1 Attachment to DCPS Entities

**Spec:** §8.1.5.1, S. 183 (PDF) — die DataWriter/DataReader der Topics
einer DLRL-Klasse und eines Graphen verbundener Objekte hängen an einem
Publisher/Subscriber, der zum Cache gehört.

**Repo:** `crates/dlrl/src/cache/mod.rs::{CacheFactory::create_cache,
Cache::the_publisher, Cache::the_subscriber,
Cache::register_all_for_pubsub}`: jeder Cache erzeugt seinen eigenen
Publisher und/oder Subscriber; jeder DataWriter und DataReader des
Caches gehört zu ihnen. Mit PRESENTATION GROUP und `coherent_access` am
Publisher ist ein `CacheAccess::write` ein kohärenter Satz über die
Topics; ein Subscriber mit GROUP und `coherent_access` nimmt die Samples
aller DataReader in einem `begin_access`/`end_access`
(`Cache::take_incoming`), so dass ein Graph in einer Update-Runde
ankommt.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_1_all_writers_and_readers_of_a_cache_belong_to_its_one_publisher_and_subscriber,
dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round}` (20 Graphen,
das verweisende Objekt vor seinem Ziel geschrieben: beim Melden jedes
Objekts ist sein Ziel da, jede Runde hat beide); grün auf dem
Linux-Testhost. Gegenprobe ohne PRESENTATION: 2 von 5 Läufen rot.

**Status:** done

### §8.1.5.2 Creation of DCPS Entities

**Spec:** §8.1.5.2, S. 183 (PDF) — Operationen auf DLRL-Ebene erzeugen
und aktivieren die DCPS-Entities eines Caches für Publikation und/oder
Subskription; Aktivieren der Subskription heißt subskribieren.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::register_home,
Cache::register_all_for_pubsub, Cache::enable_all_for_pubsub,
Cache::pubsub_state, DcpsState, CacheUsage}`:
`register_all_for_pubsub` erzeugt Topic, DataWriter und DataReader je
Topic der registrierten Klassen, nicht aktiviert
(`autoenable_created_entities` FALSE), `BadHomeDefinition` bei einer
Klasse, deren Bezugsklasse keine Home hat; `enable_all_for_pubsub`
aktiviert sie. Scheitert `register_all_for_pubsub`, bleibt keine Entity
des Aufrufs zurück und der Zustand bleibt INITIAL: die DCPS-Typen aller
Topics entstehen vor der ersten Entity, und lehnt DCPS eine Entity ab,
werden die vorher erzeugten wieder gelöscht.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_2_register_creates_and_enable_activates_the_dcps_entities,
dlrl_8_1_5_2_cache_usage_decides_publisher_subscriber_or_both,
dlrl_8_1_5_2_a_refused_topic_leaves_no_dcps_entity}`; grün
auf dem Linux-Testhost.

**Status:** done

### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, S. 183 (PDF) — QoS je DCPS-Entity, zwischen Erzeugen
und Aktivieren setzbar; DLRL bietet die DCPS-Entities der
DLRL-Entities an.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::the_publisher,
Cache::the_subscriber}` und
`crates/dlrl/src/cache/home.rs::ObjectHome::{get_topic_name,
get_all_topic_names}`; DataWriter und DataReader eines Topics über
`lookup_datawriter`/`lookup_datareader` von Publisher und Subscriber
mit dem Topic-Namen. Die DataReader werden RELIABLE erzeugt. Publisher
und Subscriber entstehen nicht aktiviert
(`crates/dcps/src/participant.rs::DomainParticipant::{create_disabled_publisher,
create_disabled_subscriber}`) und werden erst in
`enable_all_for_pubsub` aktiviert, so dass auch PRESENTATION, nach dem
Aktivieren unveränderlich, dazwischen gesetzt wird. Ein Topic ist ab dem
Erzeugen aktiviert: seine QoS wird vor `register_all_for_pubsub` gegeben
(`crates/dlrl/src/cache/mod.rs::Cache::set_topic_qos`, nur im Zustand
INITIAL, nur für ein Topic der Klassen der registrierten Homes, nur eine
konsistente QoS). Ein Topic dieses Namens und Typs, das der
DomainParticipant schon hat, behält seine QoS; weicht sie von der
gegebenen ab, erhebt `register_all_for_pubsub` `DCPSError`.
`crates/dlrl/src/cache/mod.rs::Cache::set_new_topic_qos` gibt die QoS
eines Topics nur für den Fall, dass `register_all_for_pubsub` es
erzeugt; ein Topic, das der DomainParticipant dann schon hat, wird mit
seiner QoS verwendet (die DLRL-Ports von `dds4ccm-1.1` §8.2.2 prüfen es
gegen ihr QoS-Profil). Ein späterer Aufruf für ein Topic ersetzt einen
früheren.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_3_qos_set_between_register_and_enable_reaches_the_writer,
dlrl_8_1_5_3_the_qos_of_a_topic_is_given_before_register_all_for_pubsub,
dlrl_8_1_5_3_a_qos_for_a_new_topic_leaves_a_topic_that_exists_as_it_is,
dlrl_8_1_5_3_presentation_set_between_register_and_enable_reaches_publisher_and_subscriber,
dlrl_8_1_5_3_two_attributes_of_one_object_take_different_qos,
dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round}`;
`crates/dcps/tests/create_disabled_group.rs::dcps_2_2_3_20_a_publisher_and_subscriber_created_disabled_take_presentation_before_enable`;
grün auf dem Linux-Testhost.

**Status:** done

## §8.1.6 Functional Mapping

### §8.1.6.1.1 DLRL Requested Functions — Publishing Application

**Spec:** §8.1.6.1.1, S. 184 (PDF) — Objekte erzeugen, ändern,
zerstören, die Publikation der Änderungen anfordern; nebenläufige
Änderungsstränge konsistent behandeln.

**Repo:** `crates/dlrl/src/cache/home.rs::ObjectHome::create_object`,
`crates/dlrl/src/cache/object.rs::ObjectRoot::{set, destroy}`,
`crates/dlrl/src/cache/access.rs::CacheAccess::write` (schreibt neue
und geänderte Zeilen, disponiert entfallene und alle Zeilen eines
zerstörten Objekts; ein kohärenter Satz, wenn PRESENTATION des
Publishers `coherent_access` hat); ein CacheAccess je Änderungsstrang.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_1_created_modified_and_destroyed_objects_reach_a_second_cache,
dlrl_8_1_6_1_1_two_write_accesses_in_two_threads_publish_both_sets}`;
`crates/dlrl/tests/cache.rs::dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round`
(der kohärente Satz); grün auf dem Linux-Testhost.

**Status:** done

### §8.1.6.1.2 DLRL Requested Functions — Subscribing Application

**Spec:** §8.1.6.1.2, S. 184-185 (PDF) — Objekte laden, Attribute und
Relationen lesen, navigieren, über Änderungen informiert werden;
§8.1.6.1.2.1 nur in den bekannten Subskriptionen (NotFound),
§8.1.6.1.2.2 transparente Aktualisierung, abschaltbar,
§8.1.6.1.2.3 mehrere Updates als Ganzes, geänderte Attribute.

**Repo:** `crates/dlrl/src/cache/mod.rs` (Update-Runden:
`Cache::{enable_updates, disable_updates, load, attach_listener,
rejected_updates}`, `CacheListener`),
`crates/dlrl/src/cache/home.rs::{ObjectHome::{find_object,
get_objects, get_created_objects, get_modified_objects,
get_deleted_objects, attach_listener}, ObjectListener}`,
`crates/dlrl/src/cache/object.rs::ObjectRoot::{get, get_reference,
get_references, is_attribute_modified, is_modified, read_state}`.
Im manuellen Modus registriert der Cache je Instanz nur das letzte
Update (eine Runde vergleicht nur den Zustand vor und nach ihr); die
registrierten Updates sind so viele wie die Instanzen, nicht wie die
Samples. `rejected_updates` hält höchstens `MAX_REJECTED_UPDATES` (1024)
Vermerke, die jüngsten; der erste Vermerk nennt die Zahl der verworfenen
älteren.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_2_incoming_data_become_objects_and_fire_the_listeners,
dlrl_8_1_6_1_2_1_a_relation_to_a_class_without_subscription_is_not_found,
dlrl_8_1_6_1_2_1_a_relation_to_an_object_that_is_gone_is_not_found,
dlrl_8_1_6_1_2_2_updates_apply_transparently_until_disabled,
dlrl_8_1_6_1_2_3_one_round_reports_several_updates_and_the_modified_attributes}`;
`crates/dlrl/src/cache/mod.rs::tests::{dlrl_8_1_6_1_2_rejected_notes_are_bounded_and_count_what_they_drop,
dlrl_8_1_6_1_2_2_manual_mode_registers_the_last_update_of_each_instance}`;
grün auf dem Linux-Testhost.

**Status:** done

### §8.1.6.1.3 DLRL Requested Functions — Publishing and Subscribing Applications

**Spec:** §8.1.6.1.3, S. 185 (PDF) — die Anwendung steuert die Mischung
eingehender Updates und eigener Änderungen.

**Repo:** `crates/dlrl/src/cache/mod.rs::Cache::{disable_updates,
enable_updates, load}` (eingehende Updates zurückhalten und übernehmen),
`crates/dlrl/src/cache/access.rs::CacheAccess::{create_contract,
refresh, write}` (Klone, die eingehende Updates erst beim `refresh`
übernehmen; `refresh` verwirft Ungeschriebenes). Die Zeilen, die ein
Objekt in DCPS hat, sind für `write` die selbst veröffentlichten und die,
die die Tabellen des Caches für seinen Owner-Schlüssel halten
(`crates/dlrl/src/cache/mod.rs::Cache::{rows_in_dcps, publish}`): ein
von einem anderen Participant empfangenes, geklontes und zerstörtes
Objekt und die Bestandteile, die seine Zerstörung klont, werden
disponiert, ebenso das entfernte Element einer Collection eines solchen
Objekts. Eine Zeile, die ein anderer Participant geschrieben hat,
registriert der DataWriter des Caches vor dem `dispose`.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_3_disable_updates_holds_incoming_updates_while_the_application_writes,
dlrl_8_1_6_1_3_a_write_access_is_not_changed_by_incoming_updates_until_refresh,
dlrl_8_1_6_1_3_destroying_a_received_object_reaches_a_third_cache,
dlrl_8_1_6_1_3_removing_an_item_of_a_received_object_reaches_a_third_cache}`;
grün auf dem Linux-Testhost.

**Status:** done

### §8.1.6.2 DLRL Entities

**Spec:** §8.1.6.2, S. 185-188 (PDF, mit Figure 8.4 und der Tabelle auf
S. 187). Die Tabelle nennt 19 Entities (der Text spricht von 17):
CacheFactory, CacheBase, Cache, CacheAccess, CacheListener, Contract,
ObjectHome, ObjectListener, Selection, SelectionCriterion,
FilterCriterion, QueryCriterion, SelectionListener, ObjectRoot,
Collection, List, Set, StrMap, IntMap. S. 188 listet acht Exceptions
(DCPSError, BadHomeDefinition, NotFound, AlreadyExisting, AlreadyDeleted,
PreconditionNotMet, NoSuchElement, SQLError); jede trägt das
String-Attribut `message`. Außerhalb dieser Liste erhebt der Text drei
weitere: `ExpiredTimeOut` (`Cache::lock`, §8.1.6.3.4), `ReadOnlyMode`
(`CacheAccess::write` im IDL von §8.2.1.2.1) und `DDS::BadParameter`
(`FooHome::create_selection` im impliziten IDL von §8.2.1.2.2).

**Repo:** `crates/dlrl/src/metamodel.rs::DlrlEntityKind` (die 19 Namen in
der Reihenfolge der Tabelle); `crates/dlrl/src/error.rs::DlrlException`
mit den acht gelisteten und den drei weiteren Exceptions, jede mit
`message`, `spec_name()` und der Repository-ID `IDL:DDS/<Name>:1.0`
(§8.2.1.2.1 deklariert sie in `module DDS` ohne `#pragma prefix`);
`CacheAccess::write` erhebt `ReadOnlyMode`
(`crates/dlrl/src/cache/access.rs`). Die Laufzeit-Typen `CacheFactory`,
`CacheBase`, `Cache`, `CacheAccess`, `CacheListener`, `Contract`,
`ObjectHome`, `ObjectListener`, `ObjectRoot` in `crates/dlrl/src/cache/`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::dlrl_8_1_6_2_entity_table_has_the_nineteen_names_of_page_187`,
`crates/dlrl/src/error.rs::tests::{dlrl_8_1_6_2_every_exception_carries_a_message,
dlrl_8_1_6_2_repository_ids_are_in_module_dds}`,
`crates/dlrl/tests/cache.rs::dlrl_8_1_6_2_write_on_a_read_only_access_raises_read_only_mode`;
grün auf dem Linux-Testhost.

**Status:** done — Namenstabelle und Exceptions vollständig. Die
Entities als eigene Typen stehen im folgenden Item.

### §8.1.6.2 (Rest) DLRL-Entity-Klassen als eigenständige Typen

**Spec:** §8.1.6.2, S. 185-188 (PDF, Figure 8.4 und die Tabelle auf
S. 187) — die 19 Entities als Klassen mit ihren Attributen und
Assoziationen; der Text zu `Cache`: ein Publisher und ein Subscriber
gehören zu genau einem Cache, nur Objekte eines Caches stehen in
Relation. Dazu die Werttypen des PSM (§8.2.1.2.1), auf die die
Attribute verweisen.

**Repo:** `crates/dlrl/src/cache/mod.rs::{CacheFactory, CacheBase, Cache,
CacheListener, CacheDescription, CacheUsage, CacheKind, DcpsState}`,
`crates/dlrl/src/cache/access.rs::{CacheAccess, Contract}`,
`crates/dlrl/src/cache/home.rs::{ObjectHome, ObjectListener}`,
`crates/dlrl/src/cache/object.rs::{ObjectRoot, ObjectState, ObjectScope}`,
`crates/dlrl/src/cache/selection.rs::{Selection, SelectionCriterion,
FilterCriterion, QueryCriterion, SelectionListener}`,
`crates/dlrl/src/cache/collection.rs::{Collection, List, Set, StrMap,
IntMap}`; `crates/dlrl/src/types.rs::{DlrlOid, ReferenceScope,
CriterionKind, MembershipState, RelationKind, RelationDescription,
LongSeq, ClassName, CacheName, RelationName, RelatedObjectDepth,
UNLIMITED_RELATED_OBJECTS}`. Ein Objekt ist eine Speicherzelle je Cache
oder CacheAccess (`ObjectRoot` ist ein geteilter Griff). Jeder Cache
erzeugt seinen eigenen Publisher und Subscriber.
`ObjectRoot::set_reference` und die `*_object`-Operationen der
Collections weisen ein Objekt eines anderen Caches und ein Objekt ohne
oid (aus `create_unregistered_object`, vor `register_object`) mit
`PreconditionNotMet` ab. Abweichungen: `CacheListener`,
`ObjectListener`, `FilterCriterion`, `SelectionListener` und
`Collection` sind Traits; `CacheBase` und `SelectionCriterion` sind
Enums über ihre Unterklassen; die `keys` von `IntMap` sind wie im IDL
ein `LongSeq` (die Tabelle von §8.1.6.3.19 nennt `string[]`).

**Tests:** `crates/dlrl/tests/entities.rs::{dlrl_8_1_6_2_figure_8_4_associations_navigate,
dlrl_8_1_3_1_two_references_to_one_oid_are_the_same_cell,
dlrl_8_1_6_2_a_publisher_and_a_subscriber_belong_to_one_cache,
dlrl_8_1_6_2_a_relation_across_two_caches_is_refused}`,
`crates/dlrl/src/types.rs::tests::dlrl_8_2_1_relation_description_kind_follows_its_subtype`,
`crates/dlrl/tests/cache.rs::dlrl_8_1_5_2_cache_usage_decides_publisher_subscriber_or_both`,
`crates/dlrl/tests/entity_tables.rs::dlrl_8_1_6_3_7_auto_deref_false_loads_the_state_on_access`
(das Attribut `auto_deref` von `ObjectHome`, Figure 8.4);
grün auf dem Linux-Testhost.

**Status:** done — alle 19 Entities sind Typen mit ihren Attributen und
Assoziationen, auch `ObjectHome::auto_deref` (Item §8.1.6.3 Rest).

### §8.1.6.3 Details on DLRL Entities

**Spec:** §8.1.6.3, S. 188-207 (PDF) — je Entity eine Tabelle mit
Attributen und Operationen, §8.1.6.3.1 bis §8.1.6.3.19 (der Text sprach
von 17). Die Tabellen geben nur die funktionalen Rückgabewerte an; die
Fehler legt das PSM fest (S. 188). §8.1.6.4: Reihenfolge der Listener
einer Update-Runde.

**Repo:** je Unterabschnitt:
§8.1.6.3.1 CacheFactory, §8.1.6.3.2 CacheBase und §8.1.6.3.4 Cache in
`crates/dlrl/src/cache/mod.rs`; §8.1.6.3.3 CacheAccess und §8.1.6.3.6
Contract in `crates/dlrl/src/cache/access.rs`; §8.1.6.3.5
CacheListener in `crates/dlrl/src/cache/mod.rs`; §8.1.6.3.7
ObjectHome (mit `create_selection`, `delete_selection`, `selections`,
`content_filter` nach Annex B in `crates/dlrl/src/criterion.rs`) und
§8.1.6.3.8 ObjectListener (die drei Operationen geben `bool` zurück,
TRUE auf allen Listenern einer Ebene beendet die Weitergabe an die
Homes der Elternklassen) in `crates/dlrl/src/cache/home.rs`;
§8.1.6.3.9 bis §8.1.6.3.13 Selection, SelectionCriterion,
FilterCriterion, QueryCriterion, SelectionListener in
`crates/dlrl/src/cache/selection.rs`; §8.1.6.3.14 ObjectRoot mit
`read_state` und `write_state` nach Figure 8.5 und 8.6 (VOID, NEW,
NOT_MODIFIED, MODIFIED, DELETED) in `crates/dlrl/src/cache/object.rs`;
§8.1.6.3.15 bis §8.1.6.3.19 Collection, List, Set, StrMap, IntMap mit
`added_elements`, `removed_elements`, `modified_elements` der letzten
Update-Runde oder des letzten `refresh` in
`crates/dlrl/src/cache/collection.rs`; §8.1.6.4 in
`Cache::notify` (Erzeugung: erst ObjectListener, dann Selections;
Änderung und Löschung: erst Selections, dann ObjectListener). Die
Exceptions in `crates/dlrl/src/error.rs::DlrlException`.
`ObjectListener::on_object_created` erhält wie im IDL von §8.2.1.2.2
das `ObjectRoot`, nicht die `ObjectReference` der PIM-Tabelle.
`QueryCriterion::set_query` und `set_parameters` geben TRUE zurück;
ein Fehler von Ausdruck oder Parametern ist der `SQLError` des IDL.
`delete_selection` löscht mit der Selection ihr Kriterium (§8.1.6.3.7,
S. 196): ein gelöschtes `QueryCriterion` erhebt bei `set_query` und
`set_parameters` `AlreadyDeleted` und ändert nichts (Item §8.2.1),
`create_selection` weist es mit `BadParameter` ab; ein `FilterCriterion`
ist ein Objekt der Anwendung, die Selection gibt ihre Referenz darauf
frei. Bei einem Klon in einem CacheAccess gelten
`added_elements`, `removed_elements` und `modified_elements` gegenüber
dem vorigen `refresh`.
`disable_updates` in der Mitte einer Update-Runde (§8.1.6.3.4, S. 192)
unterbricht sie (`crates/dlrl/src/cache/mod.rs::{Cache::disable_updates,
Cache::last_round_interrupted}`): `last_round_interrupted` ist TRUE, wenn
`on_end_updates` gerufen wird, und wird mit der nächsten Runde mit
Listenern wieder FALSE. Der Text sagt, der Listener erhalte
`end_updates` „mit einem Parameter“, der die Unterbrechung anzeigt; die
Tabelle von §8.1.6.3.5 und das IDL von §8.2.1.2.1 geben
`on_end_updates` keinen Parameter. Gebaut ist die Signatur von Tabelle
und IDL; der `CacheListener` liest die Anzeige in `on_end_updates` am
Cache (Erweiterung). In `on_begin_updates` gerufen, bleiben die Updates
der Runde registriert, und kein ObjectListener hört sie; später in der
Runde sind sie schon angewandt (eine Runde wendet ihre Updates vor dem
ersten ObjectListener an), und die Listener hören den Rest der Runde.

**Tests:** `crates/dlrl/tests/selections.rs::{dlrl_8_1_6_4_selections_and_object_listeners_in_the_order_of_creation_modification_deletion,
dlrl_8_1_6_3_9_a_selection_without_auto_refresh_changes_only_on_refresh,
dlrl_8_1_6_3_9_concerns_contained_reports_a_modified_constituent,
dlrl_8_1_6_3_9_load_updates_an_auto_refresh_selection_without_listener,
dlrl_8_1_6_3_12_set_parameters_and_set_query_refresh_an_auto_refresh_selection,
dlrl_8_1_6_3_7_create_and_delete_selection_check_their_preconditions,
dlrl_8_1_6_3_8_object_listener_true_stops_parent_listeners}`;
`crates/dlrl/tests/collections.rs::{dlrl_8_1_6_3_16_list_operations_and_the_changes_of_a_round,
dlrl_8_1_6_3_17_set_operations_and_the_changes_of_a_round,
dlrl_8_1_6_3_18_str_map_operations_and_the_changes_of_a_round,
dlrl_8_1_6_3_19_int_map_operations_and_the_changes_of_a_round,
dlrl_8_1_6_3_15_the_changes_of_a_clone_are_those_of_the_last_refresh}`;
`crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_2_incoming_data_become_objects_and_fire_the_listeners,
dlrl_8_1_6_1_3_a_write_access_is_not_changed_by_incoming_updates_until_refresh}`;
`crates/dlrl/tests/fixed_dcps_model.rs::dlrl_8_1_4_5_a_fixed_dcps_model_is_read_and_written_as_objects`
(`register_object` mit vorhandenem Schlüssel erhebt `AlreadyExisting`);
`crates/dlrl/src/error.rs::tests::dlrl_8_1_6_2_every_exception_carries_a_message`;
`crates/dlrl/tests/entity_tables.rs::{dlrl_8_1_6_3_4_disable_updates_in_on_begin_updates_interrupts_the_round,
dlrl_8_1_6_3_4_disable_updates_from_an_object_listener_interrupts_the_round}`;
grün auf dem Linux-Testhost.

**Status:** done — §8.1.6.3.1 bis §8.1.6.3.19 und §8.1.6.4 sind gebaut,
auch die Unterbrechung einer Runde durch `disable_updates`, angezeigt
durch `Cache::last_round_interrupted` statt durch den Parameter von
`end_updates`, den Tabelle und IDL nicht haben; `lock`, `unlock` und die Umwandlung einer `ObjectReference`
(§8.1.6.3.4), `auto_deref` mit `set_auto_deref`, `deref_all` und
`underef_all` (§8.1.6.3.7) und `which_contained_modified` (§8.1.6.3.14)
stehen im folgenden Item.

### §8.1.6.3 (Rest) Methodensignaturen der Entity-Tabellen

**Spec:** §8.1.6.3.4, S. 191-193 (PDF): `lock(to_in_milliseconds)` mit
`ExpiredTimeOut`, `unlock`, die Umwandlung einer `ObjectReference` in ein
`ObjectRoot`; §8.1.6.3.7, S. 194-196: `auto_deref`, `set_auto_deref`,
`deref_all`, `underef_all`; §8.1.6.3.14, S. 200-201:
`which_contained_modified`.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::lock, Cache::unlock,
Cache::deref_reference, Gate}`: `lock` nimmt dieselbe Sperre, die jede
Update-Runde, `load`, `refresh`, `enable_updates` und
`Selection::refresh` nehmen; andere Threads und die Runden des
Cache-Threads warten, Samples bleiben in den DataReadern und bilden die
nächste Runde. `ExpiredTimeOut`, wenn die Frist abläuft; der haltende
Thread darf erneut sperren und entsperrt ebenso oft. Abweichungen ohne
Vorgabe im Text: eine negative Frist ist `BadParameter`, `unlock` durch
einen Thread ohne Sperre ist `PreconditionNotMet`. `delete_cache`
schließt die Sperre, bevor es den Thread des Caches beendet: eine
geschlossene Sperre erhält niemand mehr, und wer auf sie wartet, hört auf
zu warten. `delete_cache` kehrt damit zurück, auch wenn der rufende oder
ein anderer Thread die Sperre hält; ein wartendes `lock` erhebt
`AlreadyDeleted`, `unlock` gibt die Sperre zurück und erhebt dann
`AlreadyDeleted`. Ruft ein `CacheListener` `delete_cache` in
`on_begin_updates` auf dem Thread des Caches, kehrt es zurück; die Runde
wendet nichts mehr an, ruft keinen weiteren Listener außer
`on_end_updates` und endet ohne Panic. Ruft es ein `ObjectListener` oder
ein `SelectionListener`, ruft die Runde keinen weiteren Listener außer
`on_end_updates`: nicht den nächsten Listener desselben Homes, nicht die
der Homes der Elternklassen, und sie prüft keine weitere Selection. `delete_cache` verwirft die
registrierten Updates, und `load`, `enable_updates` und die Runden prüfen
nach jedem Sperren des Zustands, ob der Cache gelöscht ist.
`deref_reference` ist die unbenannte Umwandlung (S. 193): das
`ObjectRoot` des Caches mit geladenem Zustand, `NotFound` ohne solches
Objekt. `crates/dlrl/src/cache/home.rs::{ObjectHome::auto_deref,
set_auto_deref, deref_all, underef_all}`: bei FALSE trägt ein
eintreffendes Objekt nur Klasse und oid, der Zustand bleibt in den
Zeilen der DCPS-Tabellen des Caches und wird beim Zugriff der Anwendung
geladen (`get`, `data`, `get_reference`, `get_references`, Inhalt einer
Collection, `deref_reference`); `ObjectRoot::is_dereferenced` zeigt das
(Erweiterung). Die Update-Runde baut den Zustand vor der Runde aus den
Zeilen nach, um die geänderten Attribute zu bestimmen; read_state,
`is_<attribute>_modified`, Listener, Selections (ein `QueryCriterion`
lädt nicht) und die Klone eines CacheAccess arbeiten wie bei TRUE; ein
gelöschtes Objekt behält seinen letzten Zustand. `set_auto_deref(TRUE)`
lädt alle Objekte des Homes; `auto_deref` gilt für die Objekte der
Klasse des Homes, abgeleitete Klassen haben ihr eigenes Home.
`crates/dlrl/src/cache/object.rs::ObjectRoot::which_contained_modified`:
je Kompositionsrelation mit einem enthaltenen Objekt, für das
`is_modified(CONTAINED_OBJECTS_SCOPE)` gilt, Name und bei `List` der
Index, bei `IntMap` und `StrMap` der Schlüssel; eine `Set`-Relation
einmal als `RelationDescription::Set` mit der Erweiterung
`RelationKind::SetRelation` (`crates/dlrl/src/types.rs`), weil
`RelationKind` des PSM keinen Wert für eine Menge hat.

**Tests:** `crates/dlrl/tests/entity_tables.rs::{dlrl_8_1_6_3_4_lock_holds_off_rounds_and_other_threads,
dlrl_8_1_6_3_4_lock_gives_several_accesses_one_state,
dlrl_8_1_6_3_4_delete_cache_by_the_holder_of_the_lock_returns,
dlrl_8_1_6_3_4_delete_cache_while_another_thread_holds_the_lock_returns,
dlrl_8_1_6_3_4_delete_cache_in_on_begin_updates_ends_the_round,
dlrl_8_1_6_3_4_delete_cache_in_an_object_listener_calls_no_further_listener,
dlrl_8_1_6_3_4_delete_cache_in_a_selection_listener_calls_no_further_listener,
dlrl_8_1_6_3_4_an_object_reference_becomes_its_object_root,
dlrl_8_1_6_3_7_auto_deref_false_loads_the_state_on_access,
dlrl_8_1_6_3_14_which_contained_modified_names_relation_index_and_key}`,
`crates/dlrl/src/types.rs::tests::dlrl_8_2_1_relation_description_kind_follows_its_subtype`;
grün auf dem Linux-Testhost.

**Status:** done — die Operationen von §8.1.6.3.4, §8.1.6.3.7 und
§8.1.6.3.14, die im Item §8.1.6.3 fehlten, sind gebaut.

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time Entities

**Spec:** §8.2.1.1, S. 211-212 (PDF), die Abbildungsregeln: Entities,
die Zugangspunkte zu Diensten sind (etwa Cache), sind lokale
Interfaces, Anwendungsobjekte und ihre Teile Valuetypes; ein Fehler, der
das spätere Verhalten beeinflusst, ist eine Exception, eine Warnung
ohne Folgen (etwa das Entfernen aus einer Liste, was dort nicht steht)
ein Rückgabewert; `get_instance` von CacheFactory ist eine statische
Operation der Sprache und steht nicht im IDL; native Typen stehen als
typedef mit einem `#define` mit Suffix `_TYPE_NATIVE`; `AlreadyDeleted`
kann jede Operation und jedes Attribut erheben und steht deshalb in
keiner raises-Klausel. §8.1.6.2, S. 188: `AlreadyDeleted`, wenn eine
Operation auf einem bereits gelöschten Objekt aufgerufen wird.
`DLRL_initialize` kommt in formal/07-01-01 nicht vor; der Einstieg ist
`CacheFactory::get_instance`.

**Repo:** `crates/dlrl/idl/dds_dlrl.idl` (die Arten von §8.2.1.1, das
IDL selbst im folgenden Item); `crates/dlrl/src/cache/mod.rs::CacheFactory::get_instance`
(statisch, bei jedem Aufruf dieselbe Fabrik); `crates/dlrl/src/types.rs::DlrlOid`
(drei `i32` für `DLRL_OID_TYPE_NATIVE long`). Warnung als Rückgabewert
(`crates/dlrl/src/cache/collection.rs`): `remove` von `List`, `Set`,
`StrMap`, `IntMap` und `Set::remove_object` geben FALSE zurück, wenn
nichts zu entfernen war; das IDL deklariert diese `remove` als `boolean`
(Änderung 9 von `dds_dlrl.idl`, im impliziten IDL `FooSet::remove`; der
Druck hat `void`). Vorher prüft jede schreibende Operation einer
Collection das Objekt wie `ObjectRoot::set`: `AlreadyDeleted` für ein
zerstörtes Objekt, `PreconditionNotMet` für ein Objekt, das nicht in
einem CacheAccess liegt, der Schreiben erlaubt (ein Objekt des Caches);
ebenso `Set::add` eines vorhandenen Elements. `get` eines fehlenden
Elements erhebt `NoSuchElement`. `AlreadyDeleted`
(`crates/dlrl/src/cache/{mod.rs, access.rs, selection.rs, home.rs,
object.rs, collection.rs}`): jede Operation eines gelöschten Cache
(`delete_cache`, das auch seine CacheAccesses löscht), CacheAccess
(`delete_access`), Contract (`delete_contract`, `purge`, Löschen des
CacheAccess), einer Selection (`delete_selection`; ebenso `refresh` einer
Selection eines gelöschten Cache) und ihres `QueryCriterion` erhebt es,
ebenso das zweite Löschen, `create_object`, `create_unregistered_object`
und `register_object` mit einem gelöschten CacheAccess und
`find_object`, `get_objects`, `get_created_objects`,
`get_modified_objects` und `get_deleted_objects` mit einem gelöschten
Cache oder CacheAccess als Quelle. Ein zerstörtes Objekt (write_state
DELETED: nach `destroy`, auch nach dem `write`, das die Zerstörung
veröffentlichte, und nach `purge` oder Löschen eines CacheAccess, der
Schreiben erlaubt) erhebt es aus jeder Operation: `destroy`,
`is_modified`, `which_contained_modified`, die erzeugten
`get_<attribute>`, `set_<attribute>` und `is_<attribute>_modified`
(`get`, `set`, `is_attribute_modified`), die Navigation, `get_list`,
`get_set`, `get_str_map`, `get_int_map` und jede Operation seiner
Collections, auch über vorher geholte Griffe. `Cache::attach_listener`,
`detach_listener`, `enable_updates`, `disable_updates`,
`find_home_by_name`, `find_home_by_index`, `CacheAccess::purge`,
`Contract::set_depth`, `set_scope`, `Selection::set_listener`,
`ObjectRoot::is_modified`, `which_contained_modified`,
`is_attribute_modified`, `Set::contains`, `added_elements`,
`removed_elements` und `modified_elements` der Collections und die vier
Listen-Operationen von `ObjectHome` geben dafür ein `Result` zurück. Die
Attribute eines gelöschten Entity und eines zerstörten Objekts behalten
ihren letzten Wert, auch `length`, `values` und `keys` einer Collection;
§8.2.1.1 nennt Attribute als Stellen, an denen `AlreadyDeleted` erhoben
werden kann, §8.1.6.2 verlangt es für Operationen. Ein Objekt im
read_state DELETED (von einem anderen Teilnehmer gelöscht oder bei einem
`refresh` nicht mehr unter Vertrag) ist nicht zerstört: die Listener
einer Löschung und `get_deleted_objects` geben es der Anwendung, und
seine Operationen lesen seinen letzten Zustand (§8.1.6.4). Die
Repository-IDs `IDL:DDS/<Name>:1.0` in `crates/dlrl/src/error.rs`.

**Tests:** `crates/dlrl/tests/psm.rs::{dlrl_8_2_1_1_access_points_are_local_interfaces_and_objects_are_valuetypes,
dlrl_8_2_1_1_get_instance_is_not_in_the_idl_and_exists_in_rust,
dlrl_8_2_1_1_a_warning_is_a_return_value,
dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_deleted_entity,
dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_destroyed_object,
dlrl_8_2_1_1_remove_checks_the_object_before_the_warning}`
(ruft auf je einem gelöschten Cache, CacheAccess, Contract, einer
Selection und einem QueryCriterion jede Operation, die das IDL dem
Interface mit seinen Basen gibt, und verlangt, dass die gerufenen
Operationen genau die des IDL sind; ruft auf einem zerstörten Objekt und
seinen Collections jede Operation, die das IDL dem Valuetype gibt, und
die erzeugten Operationen, vor und nach dem `write`),
`crates/dlrl/tests/selections.rs::dlrl_8_1_6_3_7_create_and_delete_selection_check_their_preconditions`,
`crates/dlrl/src/types.rs::tests::dlrl_8_2_1_relation_description_kind_follows_its_subtype`,
`crates/dlrl/src/error.rs::tests::dlrl_8_1_6_2_repository_ids_are_in_module_dds`;
grün auf dem Linux-Testhost.

**Status:** done — die Regeln von §8.2.1.1 gelten für die Rust-API und
das IDL; `AlreadyDeleted` erhebt jede Operation eines gelöschten Entity
und eines zerstörten Objekts; die Attribute behalten ihren letzten Wert.

### §8.2.1 (Rest) IDL-PSM-Emission + DLRL_initialize

**Spec:** §8.2.1.2, S. 212-228 (PDF): §8.2.1.2.1 das IDL der
generischen Entities, §8.2.1.2.2 das implizite IDL je Anwendungsklasse
`Foo`: `FooSeq`, `FooListener`, `FooListenerSeq`,
`FooSelectionListener`, `FooFilter`, `FooQuery`, `FooSelection`,
`FooSelectionSeq`, `FooHome`, `FooList`, `FooSet`, `FooStrMap`,
`FooIntMap`. `DLRL_initialize` gibt es in der Spezifikation nicht (siehe
Item §8.2.1). Das gedruckte IDL hat Fehler, die ein Compiler findet:
`contracted_object.` statt `;`, `ReadOnlyMode` und `DDS::BadParameter`
erhoben und nicht deklariert, `CacheBase` in `FooHome` nicht
qualifiziert, und `FooListener` und `FooSelectionListener` deklarieren
`on_object_created`, `on_object_deleted` und `on_object_out` der Basis
erneut, was IDL 4.2 §7.4.3 verbietet.

**Repo:** `crates/dlrl/idl/dds_dlrl.idl`: die Deklarationen von
§8.2.1.2.1 in eigenem Layout; der Kopf nennt neun Änderungen gegen den
Druck (das Semikolon; `ReadOnlyMode`, `ExpiredTimeOut`, `BadParameter`
deklariert; die drei Listener-Operationen in den Kommentarblöcken der
typisiert erzeugten Operationen, wo `on_object_modified` und
`on_object_in` schon stehen; `create_contract` gibt den Contract zurück
wie die Tabelle von §8.1.6.3.3; `Cache::load`, `lock`, `unlock` der
Tabelle von §8.1.6.3.4 deklariert; `delete_cache` erhebt
`PreconditionNotMet` und `DCPSError`; der Parameter `object` als
`_object`, weil `object` nur in der Schreibung vom Schlüsselwort
`Object` abweicht; `CacheDescription` als struct, weil ein Valuetype
den lokalen `DomainParticipant` nicht halten kann; `remove` von `List`,
`StrMap` und `IntMap` gibt `boolean` zurück, die Warnung von §8.2.1.1,
für die `void` keinen Platz hat).
`crates/dlrl/idl/dds_dcps.idl`: `StringSeq` und die Entity-Interfaces
`DomainParticipant`, `Publisher`, `Subscriber` ohne Operationen.
`crates/dlrl/tests/psm.rs::binding` bindet jede Deklaration (Typ,
Enumerator, Member, Attribut, Operation, Exception, Konstante) an ihr
Rust-Item, so dass ein fehlendes Item den Build bricht.
`crates/dlrl-codegen/src/idl.rs::generate_implied_idl`: die dreizehn
Konstrukte für eine Klasse in ihren Modulen, mit `DDS::CacheBase`
qualifiziert, den Operationen von `FooListener` mit `boolean` wie
Tabelle und generisches IDL, `FooHome::create_selection` mit
`concerns_contained_objects` und `PreconditionNotMet`, `find_object`
mit `NotFound`, `FooSet::remove` mit `boolean` wie die `remove` des
generischen IDL. CLI: `zerodds-idlc dlrl --lang idl`
(`tools/idlc/src/dlrl.rs::{application_classes, implied_idl_file}`)
findet die Valuetypes, die direkt oder über andere Valuetypes von
`DDS::ObjectRoot` ableiten, und schreibt `<stem>_dlrl.idl`, das die
Modellbeschreibung einbindet und das implizite IDL jeder Klasse
deklariert; `--lang all` schließt es ein.

**Tests:** `crates/dlrl/tests/psm.rs::{dlrl_8_2_1_2_1_generic_idl_parses,
dlrl_8_2_1_2_1_every_idl_declaration_has_its_rust_item}`;
`crates/dlrl-codegen/src/idl.rs::tests::dlrl_8_2_1_2_implied_idl_declares_every_derived_construct`,
`crates/dlrl-codegen/tests/implied_idl.rs::{dlrl_8_2_1_2_implied_idl_for_track_track3d_radar_parses,
dlrl_8_2_1_2_the_printed_listener_redefinition_is_refused}`;
`tools/idlc/tests/dlrl.rs::{dlrl_8_2_1_2_lang_idl_writes_the_implied_idl_of_track_track3d_radar,
dlrl_8_2_1_2_lang_idl_and_all_write_what_the_file_has_input_for}`,
`tools/idlc/src/main.rs::tests::dlrl_8_2_1_2_lang_idl_asks_for_the_implied_idl_alone`;
grün auf dem Linux-Testhost. Mit einem fremden IDL-Compiler:
`crates/dlrl-codegen/tests/omniidl.rs::{dlrl_8_2_1_2_omniidl_compiles_generic_and_implied_idl,
dlrl_8_2_1_2_omniidl_refuses_the_printed_listener_redefinition}`
(omniidl 4.3.3 mit C++-Backend; im wöchentlichen Job
`integration-bridges-vendor-corba`, Szenario `dlrl-idl-omniidl`), auf
dem Linux-Testhost grün.

**Status:** done — das generische IDL liegt als Datei vor und jede
Deklaration hat ihr Rust-Item; das implizite IDL wird je
Anwendungsklasse erzeugt, von der Bibliothek und von
`zerodds-idlc dlrl --lang idl`, und vom IDL-Frontend und von omniidl
übersetzt.

### §8.2.2 Generation Process

**Spec:** §8.2.2, S. 228-234 (PDF). §8.2.2.1 (Figure 8.7): aus der
Modellbeschreibung (IDL-Valuetypes) und den Model Tags (XML) erzeugt der
DLRL-Generator die erweiterte Modellbeschreibung mit implizitem IDL, die
DLRL-Entities als IDL und die DCPS-Beschreibung; die Sprachabbildungen
machen daraus native Konstrukte. §8.2.2.2: Anwendungsklassen sind
Valuetypes, eine typisierte Sammlung ist eine Valuetype-Vorwärtsdeklaration,
die ein `templateDef` definiert. §8.2.2.3: die Model Tags folgen der DTD
von §8.2.2.3.1 (Kodierung ISO-8859-1); §8.2.2.3.2.1 bis .14 erklären
`Dlrl`, `enumDef`, `templateDef`, `associationDef`, `compoRelationDef`,
`classMapping`, `mainTopic`, `keyDescription`, `extensionTopic`,
`monoAttribute`, `multiAttribute`, `monoRelation`, `multiRelation`,
`local`. Die Beispiel-Schnipsel von §8.2.2.3.2 sind kein wohlgeformtes
XML (fehlendes `>`, `<\multiPlaceTopic>`, `class"Radar"`, die Inhalte
`SimpleOID`, `FullOID`, `FullSimpleOID` gegen die DTD); die Tests nehmen
wohlgeformte Fassungen. Das Wort „pragma“ kommt in formal/07-01-01 nicht
vor.

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree_bytes` (liest ein
XML-Dokument aus Bytes in UTF-8 oder ISO-8859-1 nach seiner Deklaration,
mit DOCTYPE-Zeile; `parse_xml_tree` behält die UTF-8-Regel von DDS-XML
§7.1.1); `crates/dlrl/src/model_tags.rs::{parse_model_tags, ModelTags}`
(Feature `model-tags`: die DTD von §8.2.2.3.1 vollständig geprüft —
Elemente, Reihenfolge `mainTopic?, extensionTopic?`, Pflicht- und
erlaubte Attribute, Aufzählungswerte von `pattern` und `content`, Zahl
der `keyField` je `content`, Text nur in `value`, `keyField`,
`valueField`; jede Abweichung ein Fehler mit dem Pfad des Elements);
`crates/dlrl/src/generation.rs::{ModelDescription, generate}` (der
Generator: Valuetypes werden Klassen, ein Basistyp, eine Enumeration oder
eine einfache Struktur ein einwertiges Attribut, ein Valuetype der
Beschreibung eine to-1-Relation, eine per `templateDef` definierte
Vorwärtsdeklaration eine to-many-Relation oder ein mehrwertiges Attribut;
`local`, `compoRelationDef`, `associationDef` und `enumDef` angewandt;
die `classMapping`-Elemente mit dem Default-Mapping von §8.1.4.3 für
alles Fehlende; Namen, die nicht auflösen, sind Fehler, auch der
`itemType` eines `templateDef`, das kein Member verwendet; ein
`templateDef` oder `enumDef`, das kein Member verwendet, ist ein Fehler;
ein `templateDef`- oder `enumDef`-Name, der nur in der Schreibung abweicht,
wird angenommen, wie §8.2.3 `stringStrMap` und `StringStrMap` schreibt).

Daneben der ältere Pragma-Dialekt (nicht aus der Spezifikation):
`crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}` liest
`#pragma DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION`;
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs` erzeugen daraus eine
Objektklasse und ein Home je Typ; CLI `tools/idlc/src/dlrl.rs`
(`zerodds-idlc dlrl --lang {cpp,csharp,java,typescript}`), unabhängig vom
AST-Pfad, über den `idl-lower` `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` im
OpenDDS-Dialekt liest (`crates/idl-lower/src/lower.rs`, Diagnose benennt
beide Lesarten).

**Tests:** Model Tags und Generator:
`crates/dlrl/tests/model_tags.rs::{dlrl_8_2_2_3_the_full_tags_of_8_2_3_3_parse,
dlrl_8_2_2_1_the_example_generates_model_and_mapping,
dlrl_8_2_2_3_the_minimal_tags_of_8_2_3_3_take_the_default_mapping,
dlrl_8_2_2_3_1_each_violation_of_the_dtd_is_refused_with_its_path,
dlrl_8_2_2_1_tags_apply_to_the_description_and_unresolved_names_are_refused}`,
`crates/xml/src/parser.rs::tests::dlrl_8_2_2_3_1_iso_8859_1_document_read_from_bytes`;
grün auf dem Linux-Testhost.

Pragma-Dialekt — Pragma-Parser:
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
Attribut der Objektklasse (§8.1.3.1, §8.1.4.3). Vorher erzeugte der
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

**Status:** partial — gebaut und auf dem Linux-Testhost grün: das Lesen
der Model Tags gegen die DTD von §8.2.2.3.1 und der Generator, der aus
Modellbeschreibung und Model Tags Modell und Mapping macht. Offen:
(1) `zerodds-idlc` macht aus den IDL-Valuetypes der Modellbeschreibung
und den Model Tags (`--tags`) Modell und Mapping; heute findet
`dlrl --lang idl` die Valuetypes mit Basis `DDS::ObjectRoot` nur für
das implizite IDL (Item §8.2.1 Rest), die vier Sprachen lesen den
Pragma-Dialekt, `DLRL::ObjectRoot` und `#include "dlrl.idl"` wie in
§8.2.3.2 erkennt es nicht. (2) Von den Ausgaben der Figure 8.7 fehlt
die DCPS-Beschreibung als IDL; das implizite IDL schreibt
`dlrl --lang idl`, die DLRL-Entities als IDL liegen als
`crates/dlrl/idl/dds_dlrl.idl` vor. (3) Die
generierten Klassen nach §8.1.6.6 (`FooHome` mit `create_object(access)`,
`FooListener`, `FooSelection`, `FooSelectionListener`, `FooFilter`,
`FooQuery`, typisierte Sammlungen) für Rust und die vier Sprachen; die
heutigen Klassen haben ein `delete_object`, das die Spezifikation nicht
kennt, und erben von `DLRL::ObjectRoot`, für das in C++, Java, C# und
TypeScript keine Laufzeit existiert (`tools/idlc/tests/dlrl_compiles.rs`
übersetzt gegen Stubs). (4) Eine Laufzeit für C++, Java, C# und
TypeScript über `crates/zerodds-c-api`.

### §8.2.3 Example

**Spec:** §8.2.3, S. 234-244 (PDF) — illustrativer Beispiel-Code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Beispiel.

## Annex B

### Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, S. 245-246 (PDF) — B.1 die Grammatik (Condition mit
AND, OR, NOT und Klammern; ComparisonPredicate in drei Formen;
BetweenPredicate mit BETWEEN und NOT BETWEEN über einer Range aus zwei
Parametern; die RelOps `=`, `>`, `>=`, `<`, `<=`, `<>`; Parameter),
B.2 die Tokens FIELDNAME (`.` in Strukturen, `[INTEGERVALUE|STRING]` in
Sammlungen), INTEGERVALUE (mit Vorzeichen und `0x`), CHARVALUE,
FLOATVALUE (mit Exponent), STRING (beginnt mit linkem oder rechtem
Anführungszeichen, endet mit rechtem), ENUMERATEDVALUE, PARAMETER `%n`
mit n < 100. Annex B nennt als Nutzer den Filter der FilterCriterion
und die Query der QueryCriterion; `check_object` der FilterCriterion ist
aber Anwendungscode (§8.1.6.3.11). Die zwei Nutzer der Grammatik sind
`QueryCriterion` (§8.1.6.3.12) und `content_filter` des `ObjectHome`
(§8.1.6.3.7, S. 196). `SQLError` bei falscher Syntax, unbekanntem Feld
oder nicht passenden Parametern (§8.1.6.2).

**Repo:** `crates/sql-filter/src/parser.rs::{Grammar::Dlrl12, parse_with}`
(die Grammatik von Annex B: LIKE, `!=`, Boolesche Literale, `null` und
`#discriminator` abgewiesen, ein Vergleich braucht ein FIELDNAME, die
Range von BETWEEN zwei Parameter, `%0` bis `%99`; STRING auch mit `` ` ``
eröffnet, ohne `''` und ohne Zeilenumbruch, im Index `[STRING]` ebenso;
Index in `[ ]` mit Vorzeichen und `0x`);
`crates/dlrl/src/criterion.rs::QueryExpression` (Ausdruck und Parameter
gegen die Klasse geprüft: jedes FIELDNAME führt über Attribute,
Strukturen, Relationen, `List`, `IntMap`, `StrMap` und `sequence` zu
einem Basistyp oder einer Enumeration; Konstanten und Parameter passen
zu ihrem Feld, eine Enumeration vergleicht nach der Position ihrer
Enumeratoren; die Zahl der Parameter ist die geforderte; jeder Parameter
ist ein Parameter-Token, auch einer, dessen `%n` nicht vorkommt;
`set_parameters`; Auswertung mit dreiwertiger Logik, ein fehlender Wert
macht das Prädikat unbekannt; `QueryGrammar::Dcps14` als benannte Option
mit LIKE, auch dort `%n` nur mit n < 100 wie in Annex B von DDS 1.4,
ein größeres n ist `SQLError`); `crates/dlrl/src/cache/home.rs::{ObjectHome::set_content_filter,
ObjectHome::content_filter}` mit der Anwendung in der Update-Runde
(`crates/dlrl/src/cache/mod.rs`); ein FIELDNAME des `content_filter`, der
über eine Relation führt, ist `SQLError`
(`QueryExpression::fields_across_relations`). Die ältere Teilmenge query.rs
(Sortierung, Limit, Closure-Filter; nicht in Annex B) ist entfernt.

**Tests:** `crates/dlrl/tests/annex_b.rs::{dlrl_b_1_every_production_parses_and_evaluates,
dlrl_b_1_like_and_order_by_are_sql_error,
dlrl_b_2_fieldname_navigates_structure_collection_and_relation,
dlrl_b_2_parameter_100_is_sql_error,
dlrl_b_2_hexadecimal_char_float_string_and_enumerated_values,
dlrl_b_2_too_few_parameters_are_sql_error,
dlrl_b_2_fields_across_relations_are_named,
dlrl_8_1_6_3_7_a_content_filter_across_a_relation_is_sql_error,
dlrl_8_1_6_3_7_content_filter_lets_only_passing_objects_into_the_cache}`,
`crates/dlrl/src/criterion.rs::tests::{dlrl_b_2_fieldname_text_splits_into_steps,
dlrl_b_2_like_of_the_dcps14_option_matches_percent_and_underscore}`,
`crates/sql-filter/src/parser.rs::tests::dlrl_b_grammar_dlrl12_refuses_what_annex_b_does_not_have`;
grün auf dem Linux-Testhost.

**Status:** done — Grammatik und Tokens vollständig, geprüft gegen die
Klasse und ausgewertet; `content_filter` nutzt sie. Der `content_filter`
wird in der Update-Runde des Caches auf das aus allen Zeilen gebaute
Objekt angewandt, nicht als ContentFilteredTopic von DCPS, weil die
Attribute eines Objekts in mehreren Topics liegen können. Ein FIELDNAME
des `content_filter` führt nicht über eine Relation: Der Filter soll auf
DCPS abgebildet werden (§8.1.6.3.7, „intended to be mapped on the
underlying DCPS infrastructure“) und entscheidet über die Daten des
ankommenden Objekts; ein Feld eines anderen Objekts machte die
Entscheidung von dessen Ankunft abhängig. `QueryCriterion`
als Typ mit `set_query` über `QueryExpression` ist Teil des Items
§8.1.6.2 (Rest).

---

## Audit-Status

17 done / 6 partial / 0 open / 0 blocked / 4 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document dlrl-1.2.md`):

* `cargo nextest run --profile merge -p zerodds-dlrl --all-targets` (Job `merge-rust-dlrl`) — 64 Tests grün.
* `cargo nextest run --profile merge -p zerodds-dlrl-codegen --all-targets` (Job `merge-rust-dlrl-codegen`) — 9 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (Job `merge-rust-idl-lower`) — 66 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 20 Tests grün.
* `python3 ci/run_integration.py integration-codegen-csharp` (Job `integration-codegen-csharp`) — 8 Tests grün.

167 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `dlrl-1.2.open.md`.
