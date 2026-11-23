# OMG DDS 1.2 — Data Local Reconstruction Layer (DLRL) — Spec Coverage

**Spec:** [OMG DDS v1.2 — formal/07-01-01, January 2007 →](https://www.omg.org/spec/DDS/1.2/) (393 pp.). DLRL is in
**§8 Data Local Reconstruction Layer** (pp. 173-211) and **Annex B Syntax
for DLRL Queries and Filters** (p. 245).

**Note on spec versioning:** DDS 1.4 (`formal/15-04-10`) and following
revisions split out the DCPS part (`zerodds-dcps-1.4.md`); the DLRL
specification was not carried into 1.4 and thus remains, as DDS 1.2 §8, the
authoritative normative source for DLRL.

**Context.** DLRL is the optional object-oriented layer above DCPS (the
DDS-topic world → the domain-object world). Built are the model, the
structural mapping and the 19 run-time entities of the table on p. 187
over `zerodds-dcps`, among them `Selection` with its criteria and the
collection classes, with all operations of the tables of §8.1.6.3, and
the IDL of the PSM (§8.2.1) with the implied IDL of each application
class. Open are the generated typed classes (§8.2.2). `ObjectCache` and `Transaction` are
an older subset without a counterpart in §8.
Spread across:

- `crates/dlrl/` — DLRL runtime (model and type enforcement, structural mapping with rows; run-time entities over DCPS; ObjectCache, Transaction, Annex B expressions, pragma parsing)
- `crates/dlrl-codegen/` — code generation for the Generation Process (§8.2.2)
- `tools/idlc/src/dlrl.rs` — CLI front end (`zerodds-idlc dlrl`), wiring
  `dlrl-codegen` in as a subcommand; see §8.2.2 for the pragma-name
  collision with OpenDDS's `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` dialect

**Crate mapping:**

| Spec area | Crate / module |
|---|---|
| §8.1.3 What can be modeled (model, type enforcement) | `crates/dlrl/src/model.rs`, `crates/dlrl/src/types.rs` |
| §8.1.4 Structural Mapping | `crates/dlrl/src/mapping.rs` (mapping, default mapping, topic descriptions), `crates/dlrl/src/rows.rs` (rows) |
| §8.1.5 Operational Mapping | `crates/dlrl/src/cache/mod.rs` (the DCPS entities of a Cache), `crates/dlrl/src/dcps_binding.rs` (DCPS types, row ↔ sample) |
| §8.1.6 Functional Mapping | `crates/dlrl/src/cache/` (Cache, CacheAccess, ObjectHome, ObjectRoot, listeners; `selection.rs` Selection and criteria; `collection.rs` List, Set, StrMap, IntMap), PSM value types in `crates/dlrl/src/types.rs` |
| §8.2.1 IDL PSM | `crates/dlrl/idl/` (generic IDL), `crates/dlrl-codegen/src/idl.rs` (implied IDL), CLI `tools/idlc/src/dlrl.rs` (`--lang idl`) |
| §8.2.2 Model tags and generator | `crates/dlrl/src/model_tags.rs`, `crates/dlrl/src/generation.rs`; pragma dialect `crates/dlrl/src/pragma.rs` |
| §8.2.2 Generation Process (codegen) | `crates/dlrl-codegen/`, CLI: `tools/idlc/src/dlrl.rs` |
| Annex B Query/Filter Syntax | `crates/dlrl/src/criterion.rs`, grammar `Dlrl12` in `crates/sql-filter/src/parser.rs` |

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

**Status:** n/a (informative) — purpose and design rationale.

### §8.1.2 DLRL description

**Spec:** §8.1.2, p. 173 (PDF) — "With DLRL, the application developer will
be able to: Describe classes of objects with their methods, data fields and
relations; Attach some of those data fields to DCPS entities; Manipulate
those objects (i.e., create, read, write, delete) using the native language
constructs that will, behind the scenes, activate the attached DCPS entities
in the appropriate way; Have those objects managed in a cache of objects,
ensuring that all the references that point to a given object actually point
to the same language cell."

**Repo:** — (the obligations are in the items for §8.1.3 to §8.1.6).

**Tests:** —

**Status:** n/a (informative) — description; concrete requirements in
§8.1.3-§8.1.6.

## §8.1.3 What can be modeled with DLRL

### §8.1.3.1 DLRL objects

**Spec:** §8.1.3.1, pp. 174-175 (PDF) — attributes are local (not
distributed) or shared. "A DLRL object has at least one shared
attribute." Shared attributes are mono-valued (basic type, enumeration,
simple structure, reference to a DLRL object) or multi-valued (list,
map, set of homogeneously typed items). Type enforcement: strict
equality for simple types, inclusion for references (a derived class
fits a reference or a collection of its base). "two different
references to the same object [...] will actually point to the same
memory location." "Object identity is given by an oid (object ID) part
of any DLRL object."

**Repo:** `crates/dlrl/src/model.rs::{ClassModel, Attribute,
AttributeType, SimpleType, CollectionBasis, Model::new,
Model::check_value, ObjectRef}` (local attributes in
`ClassModel::locals`, the rule "at least one shared attribute" in
`Model::new`, type enforcement per value and per collection item in
`Model::check_value`, which refuses a reference to `DlrlOid::NIL` with
`TypeError::NilOid`); `crates/dlrl/src/types.rs::DlrlOid`;
`crates/dlrl/src/rows.rs::ObjectData` (the oid as part of every object
and every key). The oid a Cache allocates
(`crates/dlrl/src/cache/mod.rs::oid_creator`,
`crates/dlrl/src/rows.rs::KeyAssociation::process_wide`): first value the
host id of the GUID prefixes, second the process id marked with its PID
namespace, third a start value from the randomness of the operating
system plus a counter that all Caches of the process share. Two Caches of
one process thus never allocate the same oid, also not on two
participants.

**Tests:** `crates/dlrl/tests/entities.rs::dlrl_8_1_3_1_two_caches_never_allocate_one_oid`;
`crates/dlrl/tests/objects.rs::{dlrl_8_1_3_1_simple_types_need_the_exact_type,
dlrl_8_1_3_1_collections_are_homogeneous_and_of_their_basis,
dlrl_8_1_3_1_a_derived_object_fits_a_reference_and_a_collection_of_its_base,
dlrl_8_1_3_1_a_class_without_shared_attribute_is_refused,
dlrl_8_1_3_1_a_local_attribute_is_not_published,
dlrl_8_1_3_1_the_oid_is_part_of_every_object,
dlrl_8_1_3_1_a_reference_to_the_nil_oid_is_refused}`; green on the
Linux test host.

**Status:** partial — the model, local and shared attributes, type
enforcement and the oid are built. Open: one memory cell per identity
(a cache with `ObjectRoot` as shared handle, item §8.1.6.2 rest) and
the call of `Model::check_value` from the `set_<attribute>` operations
of the generated classes (item §8.1.6.3 rest).

### §8.1.3.2.1 Inheritance

**Spec:** §8.1.3.2.1, p. 175 (PDF) — "Single inheritance is allowed
between DLRL objects. Any object inheriting from a DLRL object is itself
a DLRL object. ObjectRoot is the ultimate root for all DLRL objects.
DLRL objects can, in addition, inherit from any number of native
language objects."

**Repo:** `crates/dlrl/src/model.rs::{ClassModel::bases, Model::new,
Model::parent, Model::children, Model::lineage, Model::is_kind_of,
Model::native_bases, OBJECT_ROOT_NAMES}`: exactly one DLRL base
(`ObjectRoot` spelled `ObjectRoot`, `DDS::ObjectRoot` or
`DLRL::ObjectRoot`, or a class of the model), any number of native
bases; a `final` class is not extended, cycles are refused.

**Tests:** `crates/dlrl/tests/relations.rs::{dlrl_8_1_3_2_1_track3d_is_a_track_and_an_object_root,
dlrl_8_1_3_2_1_two_dlrl_bases_are_refused,
dlrl_8_1_3_2_1_every_class_descends_from_object_root_and_no_final_class_is_extended}`;
green on the Linux test host.

**Status:** partial — inheritance in the model is built. Open: the
generated classes (Track3D extends the generated class Track, item
§8.2.2) and `ObjectHome` with `parent` and `children` (item §8.1.6.2
rest).

### §8.1.3.2.2 Associations

**Spec:** §8.1.3.2.2, p. 175 (PDF) — "Supported association ends are
either to-1 or to-many. [...] Plain use-relations (no impact on the
object life-cycle); Compositions (constituent object lifecycle follows
the compound object's one). [...] One plain relation can inverse another
plain relation, providing that the types match: can make 1-1, 1-n, n-m.
One composition relation can only inverse a to-1 relation to the
compound object: can make 1-1 or 1-n."

**Repo:** `crates/dlrl/src/model.rs::{Relation, Multiplicity,
Model::new}` (to-1 and to-many on every collection basis,
`is_composition`, `inverse`; `Model::new` checks that both ends name
each other, that each end targets the owner class of the other, and
that the inverse of a composition is a plain to-1 relation);
`crates/dlrl/src/mapping.rs::{MonoRelationMapping,
MultiRelationMapping}` carry `is_composition`.

**Tests:** `crates/dlrl/tests/relations.rs::{dlrl_8_1_3_2_2_plain_inverses_make_1_1_1_n_and_n_m,
dlrl_8_1_3_2_2_inverse_relations_name_each_other_and_their_types_match,
dlrl_8_1_3_2_2_the_inverse_of_a_composition_must_be_to_1}`; green on
the Linux test host.

**Status:** partial — relations, compositions and the two rules for
inverses are built in the model. Open at run time: setting one end
updates the other (1-1, 1-n, n-m), and `destroy` of a compound destroys
its constituents (item §8.1.6.3 rest, `ObjectRoot` and the
collections).

### §8.1.3.3 Metamodel

**Spec:** §8.1.3.3, pp. 175-176 (PDF, Figure 8.1) — `Class` (`final`,
`parent`, `children`), `Attribute` with `MonoAttribute` and
`MultiAttribute`, `Relation` (`is_composition`, `inverse`) with
`MonoRelation` and `MultiRelation`, `SimpleType` (`BasicType`,
`EnumerationType`, `SimpleStructType`), `MultiSimpleType`,
`MultiRefType`, `CollectionBase` (`SetBase`, `ListBase`, `MapBase`).
BasicType: long, short, char, octet, real, double, string and sequences
of these. "This metamodel is given for explanation purpose. This
specification does not require that it is implemented as such."

**Repo:** `crates/dlrl/src/model.rs` (the mapping Figure 8.1 → Rust
types is in the module documentation): `ClassModel`,
`Attribute`/`AttributeType::{Mono, Multi}`,
`Relation`/`Multiplicity::{ToOne, ToMany}`, `SimpleType`, `BasicType`,
`BasicItem`, `EnumerationType`, `SimpleStructType`, `CollectionBasis`
(the map basis in its two PSM forms `StrMap` and `IntMap`).

**Tests:** `crates/dlrl/tests/model.rs::{dlrl_8_1_3_3_every_construct_of_figure_8_1_is_describable,
dlrl_8_1_3_3_ill_formed_types_are_refused}`; green on the Linux test
host.

**Status:** done — every construct of Figure 8.1 can be described and
reaches a DCPS description under the default mapping.

## §8.1.4 Structural Mapping

### §8.1.4.1 Design principles

**Spec:** §8.1.4.1, p. 177 (PDF) — design principles: no unnecessary data
duplication, do not impede efficiency, flexible mapping per attribute,
default mapping available.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — design principles; the default mapping and the per-attribute mapping are in the following items for §8.1.4.

### §8.1.4.2 Mapping rules

**Spec:** §8.1.4.2, pp. 177-178 (PDF) — DCPS data as tables: topic =
table, instance = row with its key, field = column. §8.1.4.2.1: each
class has a main table; "A DLRL object is considered to exist if it has
a corresponding row in this table." §8.1.4.2.2: a reference is class
name plus oid (full oid); the class name is needed when several classes
share a main table or the target of a relation has derived classes.
§8.1.4.2.3: mono-valued attributes and relations as cells of one row
keyed like the object (basic type one cell, enumeration one integer or
string cell, structure as many cells as needed, reference oid or full
oid); multi-valued ones as one row per item keyed by the owner plus an
index (integer for a list, string or integer for a map).

**Repo:** `crates/dlrl/src/rows.rs::{Mapping::rows_of,
Mapping::reconstruct, Row, Cell, ObjectData, Reconstruction,
SkipReason}`: `rows_of` writes the rows of an object, `reconstruct`
builds objects only from rows of the main table and reports rows
without a main row, with an unknown class or with a class not mapped
there in `Reconstruction::skipped`. A set has no index; its item
completes the key. A reference to no object is an empty class part
with `DlrlOid::NIL`; the nil oid is refused as the oid of an object
(`RowError::NilOid` in `rows_of`), in a reference to an object
(`TypeError::NilOid`), in the owner key of a row read
(`RowError::NilOid` in `reconstruct`), and beside a class part
(`RowError::BadCell`). The rows of a `NoOid` key are described in item
§8.1.4.5.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_2_1_an_object_exists_with_its_row_in_the_main_topic,
dlrl_8_1_4_2_2_the_class_part_is_required_for_a_shared_main_topic_and_for_a_target_with_derived_classes,
dlrl_8_1_4_2_2_the_nil_oid_designates_no_object,
dlrl_8_1_4_2_3_cells_per_attribute_kind,
dlrl_8_1_4_2_3_a_collection_is_one_row_per_item_keyed_by_owner_and_index,
dlrl_8_1_4_5_objects_over_a_keyed_topic_without_oid_fields}`; green on
the Linux test host.

**Status:** done — the three groups of rules are checked on the rows
written and read.

### §8.1.4.3 Default mapping

**Spec:** §8.1.4.3, p. 179 (PDF) — main table: topic name = class name,
oid fields `class` and `oid`; mono-valued attributes in the main table,
field name = attribute name, or attribute name and sub-field joined by
'.'; per multi-valued attribute a topic `<Class>.<attribute>` with the
extra key field `index`; derived classes with an extension table for the
added mono-valued attributes. "This default mapping is overwritten by
any mapping information provided by the application developer."

**Repo:** `crates/dlrl/src/mapping.rs::{Mapping::new,
Mapping::default_for, default_mapping}`. Every part not given (main
topic, extension topic, each attribute) takes the default: a derived
class without its own main topic lives in the main topic of its base
and gets the extension topic `<Class>` when it has mono-valued members
of its own; the key of the main topic is (`class`, `oid`) when several
classes share it, else (`oid`); reference fields are named
`<attribute>`, or `<attribute>.class` and `<attribute>.oid`. A given
part replaces the default for that part only.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_3_the_minimal_tags_of_8_2_3_3_yield_the_default_topics_and_fields,
dlrl_8_1_4_3_a_given_mapping_overrides_the_default_per_attribute}`;
green on the Linux test host.

**Status:** done — not checked: whether DCPS implementations accept '.'
in topic and field names (the default names of the specification).

### §8.1.4.4 Metamodel with mapping information

**Spec:** §8.1.4.4, pp. 180-181 (PDF, Figure 8.2) — Class:
`main_topic`, `oid_field`, `class_field`, `full_oid_required`, `final`;
MonoAttribute: `topic`, `target_field`, `key_fields`; MultiAttribute in
addition `index_field`, its topic "cannot be the same as the owner
Class::topic"; MonoRelation: `topic`, `target_fields` (1 or 2 by
`full_oid_required`), `key_fields`, `full_oid_required`,
`is_composition`; MultiRelation in addition `index_field`.
`key_fields`: 1 or 2 depending on the class definition.

**Repo:** `crates/dlrl/src/mapping.rs::{ClassMapping, MemberMapping,
MonoAttributeMapping, MultiAttributeMapping, MonoRelationMapping,
MultiRelationMapping, KeyDescription, KeyContent, Topic,
TopicDescription, ClassMappingSpec, MemberMappingSpec, TopicSpec,
MultiTopicSpec, MappingError}`; the accessors `full_oid_required`,
`oid_field`, `class_field`, `key_fields`, `target_fields`,
`index_field`. `Mapping::new` checks: key fields per content (FullOid
2, SimpleOid 1, NoOid at least 1), place and collection topics
designate the owner as the main topic does, a collection topic is
neither a topic of the owner class nor of another member, a set has no
index field, one value field of its own per cell of the type (a
repeated name is `MappingError::RepeatedValueField`), an index or set
item field that does not repeat a field of the owner key
(`MappingError::FieldConflict`), target fields matching the main topic
of the target, a class part where several classes have rows in one
topic, one topic with one key, one type name and one type per field. The tag source (XML of §8.2.2.3) follows with item §8.2.2.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_4_each_element_of_the_example_yields_its_topic_and_fields,
dlrl_8_1_4_4_a_collection_topic_differs_from_the_owner_topics,
dlrl_8_1_4_4_key_fields_follow_the_key_content,
dlrl_8_1_4_4_the_given_elements_fit_the_members}`; green on the Linux
test host.

**Status:** done — the example of §8.2.3.3 yields the five topics of
§8.2.3.4 and the main topic `RADAR-TOPIC` of Radar, which the table of
§8.2.3.4 does not list. The specification writes `Y_TOPIC` and
`RADAR_OID` (XML) and `Y-TOPIC` and `RADAR-OID` (table); the test takes
the XML names.

### §8.1.4.5 Mapping when DCPS model is fixed

**Spec:** §8.1.4.5, p. 182 (PDF) — an existing DCPS model is mapped onto
DLRL; participants at DCPS level and at DLRL level work in one system.
The topic keys identify the objects; the implementation manages the
association keys ↔ oid.

**Repo:** `crates/dlrl/src/mapping.rs::{KeyContent::NoOid,
ClassMapping::identity, Mapping::identity_cells,
MappingError::NoOidKeyField}`: each key field of the main topic of a
`NoOid` class is the value field of a mono-valued attribute of a basic
or enumeration type; the key fields of the class's other topics and the
target fields of a relation to it take its type.
`crates/dlrl/src/rows.rs::{KeyAssociation, Mapping::identity_key,
Mapping::rows_of_with, Mapping::reconstruct_with,
RowError::UnknownKey, RowError::NoObjectInNoOidReference}`: the
association key values ↔ oid; a key value gets a local oid when it is
first read and keeps it.
`crates/dlrl/src/cache/home.rs::ObjectHome::{create_unregistered_object,
register_object}` derives the oid from the key values (`AlreadyExisting`
for a key in use); `crates/dlrl/src/cache/object.rs::ObjectRoot::set`
refuses to change a key field of a registered object (§8.1.6.3.14).
A key is free again once its object no longer exists:
`crates/dlrl/src/cache/mod.rs::Cache::{clean, publish,
release_unheld_key}` (a dispose through the Cache's DataReader; an
object destroyed and written that the Cache does not hold, as in a
WRITE_ONLY Cache) and
`crates/dlrl/src/cache/access.rs::CacheAccess::{refresh, purge}` (a
registered object never written that `refresh`, `purge` or
`delete_access` drops).
Foreign names for topics, types and fields: `TopicSpec`,
`crates/dlrl/src/pragma.rs::parse_pragma`.

**Tests:** `crates/dlrl/tests/mapping.rs::{dlrl_8_1_4_3_a_given_mapping_overrides_the_default_per_attribute,
dlrl_8_1_4_5_objects_over_a_keyed_topic_without_oid_fields,
dlrl_8_1_4_5_a_key_field_must_be_an_identity_attribute}`;
`crates/dlrl/tests/fixed_dcps_model.rs::{dlrl_8_1_4_5_a_fixed_dcps_model_is_read_and_written_as_objects,
dlrl_8_1_4_5_a_key_is_free_again_once_its_object_is_gone}`
(a DCPS participant with the type `Legacy::StationType` from IDL text,
without class and oid field: samples become objects, a dispose deletes,
a registered object becomes a sample; in a WRITE_ONLY Cache the key is
free again after destroy and write, after `refresh`, `purge` and
`delete_access`, not after a write without destroy);
`crates/dlrl/tests/cyclone_fixed_model.rs::dlrl_8_1_4_5_a_fixed_cyclone_model_is_read_and_written_as_objects`
(a C program of Cyclone DDS, `crates/dlrl/tests/cyclone/station.c`, on
the same topic in both directions; `#[ignore]`, job `vendor-live-dcps`);
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma}`; green on the Linux test host, the Cyclone test
with Cyclone DDS 0.10.5.

**Status:** partial — built and measured against Cyclone DDS 0.10.5:
objects over a fixed DCPS model in both directions and the management
of the association keys ↔ oid. Not measured: against OpenSplice (not on
the Linux test host) and against Cyclone DDS 11.0.1 of the image in job
`vendor-live-dcps`. Open: inheritance between application classes with
a `NoOid` key. A derived class in the main topic of its base class is
refused (`MappingError::ClassPartRequired`), because without a class
field the class of a row is not in the topic; §8.1.4.5 calls such
inheritance tricky but feasible, and the mapping supported. Restrictions
of this implementation that the specification does not make:
`ObjectHome::create_object` of a `NoOid` class gives
`PreconditionNotMet`, because an object of `create_object` is registered
and the identity fields of a registered object may not change any more
(§8.1.6.3.7); it is made by `create_unregistered_object` and
`register_object`. A to-1 relation to a `NoOid` class that designates no
object is not written (`RowError::NoObjectInNoOidReference`), because
the target's key fields have no value for "no object".

### §8.1.4.6 How is this mapping indicated?

**Spec:** §8.1.4.6, p. 182-183 (PDF, with Figure 8.3) — two orthogonal
descriptions (the object model with its shared part, the mapping); the
generator produces the native model description, the DLRL entities and,
on demand, the DCPS description.

**Repo:** the object model `crates/dlrl/src/model.rs::Model` and the
mapping `crates/dlrl/src/mapping.rs::ClassMappingSpec` as separate
inputs; the DCPS description on demand:
`crates/dlrl/src/dcps_binding.rs::{dcps_description, topic_type}`; the
generic DLRL entities in `crates/dlrl/src/cache/`. Deviation from the
PSM: the DCPS description emits `DDS::DLRLOid` as
`@final struct DLRLOid { long value0; long value1; long value2; }`, the
PSM IDL (§8.2.1) as `struct DLRLOid { DLRL_OID_TYPE_NATIVE value[3]; }`.
The CDR octets are the same, the type (member names, structure, hence
the TypeObject) is not; an oid field of another DLRL implementation in
array form is not assignable under XTypes. Cause according to
`crates/dlrl/src/dcps_binding.rs`: an oid field is a key field, and the
key holder of `zerodds-wire-codec` does not accept an array as key
member (not measured again here). The generator from
IDL with model tags: `crates/dlrl/src/pragma.rs::parse_pragma`,
`crates/dlrl-codegen/src/lib.rs::collect_type_infos` and the backends
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs`.

**Tests:** `crates/dlrl/tests/cache.rs::dlrl_8_1_4_6_the_dcps_type_of_each_topic_carries_its_rows`;
`crates/dlrl/src/pragma.rs::tests::{parses_data_type_pragma,
parses_data_key_pragma, parses_relation_pragma}`;
`crates/dlrl-codegen/src/lib.rs::tests::{everything_about_one_type_lands_in_one_entry,
every_backend_carries_every_key_and_relation, every_home_names_its_object_class}`;
green on the Linux test host.

**Status:** partial — the two inputs and the DCPS description are
built. Open: the generator that produces from model description and
model tags the native model description and the typed DLRL entities
over `crates/dlrl/src/cache/` (item §8.2.2); `DDS::DLRLOid` in the
array form of the PSM, once `zerodds-wire-codec` accepts an array as
key member.

## §8.1.5 Operational Mapping

### §8.1.5.1 Attachment to DCPS entities

**Spec:** §8.1.5.1, p. 183 (PDF) — the DataWriters/DataReaders of the
topics of a DLRL class and of a graph of related objects are attached to
one Publisher/Subscriber that belongs to the Cache.

**Repo:** `crates/dlrl/src/cache/mod.rs::{CacheFactory::create_cache,
Cache::the_publisher, Cache::the_subscriber,
Cache::register_all_for_pubsub}`: each Cache creates its own Publisher
and/or Subscriber; every DataWriter and DataReader of the Cache belongs
to them. With PRESENTATION GROUP and `coherent_access` on the Publisher,
a `CacheAccess::write` is one coherent set across the topics; a
Subscriber with GROUP and `coherent_access` takes the samples of all
DataReaders in one `begin_access`/`end_access`
(`Cache::take_incoming`), so that a graph arrives in one update round.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_1_all_writers_and_readers_of_a_cache_belong_to_its_one_publisher_and_subscriber,
dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round}` (20 graphs,
the referring object written before its target: when each object is
reported its target is there, each round has both); green on the Linux
test host. Counter-check without PRESENTATION: 2 of 5 runs red.

**Status:** done

### §8.1.5.2 Creation of DCPS entities

**Spec:** §8.1.5.2, p. 183 (PDF) — operations at DLRL level create and
activate the DCPS entities of a Cache for publication and/or
subscription; activating the subscription is subscribing.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::register_home,
Cache::register_all_for_pubsub, Cache::enable_all_for_pubsub,
Cache::pubsub_state, DcpsState, CacheUsage}`:
`register_all_for_pubsub` creates Topic, DataWriter and DataReader per
topic of the registered classes, not enabled
(`autoenable_created_entities` FALSE), `BadHomeDefinition` for a class
whose related class has no home; `enable_all_for_pubsub` enables them.
When `register_all_for_pubsub` fails, no entity of the call is left and
the state stays INITIAL: the DCPS types of all topics are built before
the first entity, and when DCPS refuses an entity, those created before
are deleted again.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_2_register_creates_and_enable_activates_the_dcps_entities,
dlrl_8_1_5_2_cache_usage_decides_publisher_subscriber_or_both,
dlrl_8_1_5_2_a_refused_topic_leaves_no_dcps_entity}`; green
on the Linux test host.

**Status:** done

### §8.1.5.3 Setting of QoS

**Spec:** §8.1.5.3, p. 183 (PDF) — QoS per DCPS entity, settable between
creation and activation; DLRL gives access to the DCPS entities of the
DLRL entities.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::the_publisher,
Cache::the_subscriber}` and
`crates/dlrl/src/cache/home.rs::ObjectHome::{get_topic_name,
get_all_topic_names}`; the DataWriter and DataReader of a topic through
`lookup_datawriter`/`lookup_datareader` of the Publisher and Subscriber
with the topic name. The DataReaders are created RELIABLE. The Publisher
and Subscriber are created not enabled
(`crates/dcps/src/participant.rs::DomainParticipant::{create_disabled_publisher,
create_disabled_subscriber}`) and enabled only in `enable_all_for_pubsub`,
so that PRESENTATION, immutable once enabled, is also set in between. A
Topic is enabled when it is created: its QoS is given before
`register_all_for_pubsub` (`crates/dlrl/src/cache/mod.rs::Cache::set_topic_qos`,
only in the state INITIAL, only for a topic of the classes of the
registered homes, only a consistent QoS). A Topic of that name and type
the DomainParticipant has already keeps its QoS; when it differs from the
one given, `register_all_for_pubsub` raises `DCPSError`.
`crates/dlrl/src/cache/mod.rs::Cache::set_new_topic_qos` gives the QoS of
a Topic only for the case that `register_all_for_pubsub` creates it; a
Topic the DomainParticipant has by then is used with its QoS (the DLRL
ports of `dds4ccm-1.1` §8.2.2 check it against their QoS profile). A
later call for a topic replaces an earlier one.

**Tests:** `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_3_qos_set_between_register_and_enable_reaches_the_writer,
dlrl_8_1_5_3_the_qos_of_a_topic_is_given_before_register_all_for_pubsub,
dlrl_8_1_5_3_a_qos_for_a_new_topic_leaves_a_topic_that_exists_as_it_is,
dlrl_8_1_5_3_presentation_set_between_register_and_enable_reaches_publisher_and_subscriber,
dlrl_8_1_5_3_two_attributes_of_one_object_take_different_qos,
dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round}`;
`crates/dcps/tests/create_disabled_group.rs::dcps_2_2_3_20_a_publisher_and_subscriber_created_disabled_take_presentation_before_enable`;
green on the Linux test host.

**Status:** done

## §8.1.6 Functional Mapping

### §8.1.6.1.1 DLRL requested functions — publishing application

**Spec:** §8.1.6.1.1, p. 184 (PDF) — create, modify and destroy objects,
request the publication of the changes; handle concurrent threads of
modifications consistently.

**Repo:** `crates/dlrl/src/cache/home.rs::ObjectHome::create_object`,
`crates/dlrl/src/cache/object.rs::ObjectRoot::{set, destroy}`,
`crates/dlrl/src/cache/access.rs::CacheAccess::write` (writes new and
changed rows, disposes rows no longer there and all rows of a destroyed
object; one coherent set when the Publisher's PRESENTATION has
`coherent_access`); one CacheAccess per thread of modifications.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_1_created_modified_and_destroyed_objects_reach_a_second_cache,
dlrl_8_1_6_1_1_two_write_accesses_in_two_threads_publish_both_sets}`;
`crates/dlrl/tests/cache.rs::dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round`
(the coherent set); green on the Linux test host.

**Status:** done

### §8.1.6.1.2 DLRL requested functions — subscribing application

**Spec:** §8.1.6.1.2, p. 184-185 (PDF) — load objects, read attributes
and relations, navigate, be made aware of changes; §8.1.6.1.2.1 only
inside the known subscriptions (NotFound), §8.1.6.1.2.2 transparent
updates that can be switched off, §8.1.6.1.2.3 several updates as a
whole, modified attributes.

**Repo:** `crates/dlrl/src/cache/mod.rs` (update rounds:
`Cache::{enable_updates, disable_updates, load, attach_listener,
rejected_updates}`, `CacheListener`),
`crates/dlrl/src/cache/home.rs::{ObjectHome::{find_object,
get_objects, get_created_objects, get_modified_objects,
get_deleted_objects, attach_listener}, ObjectListener}`,
`crates/dlrl/src/cache/object.rs::ObjectRoot::{get, get_reference,
get_references, is_attribute_modified, is_modified, read_state}`.
In manual mode the Cache registers only the last update of each instance
(a round compares only the state before and after it); the registered
updates are as many as the instances, not as the samples.
`rejected_updates` keeps at most `MAX_REJECTED_UPDATES` (1024) notes, the
most recent ones; the first note gives the number of older ones dropped.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_2_incoming_data_become_objects_and_fire_the_listeners,
dlrl_8_1_6_1_2_1_a_relation_to_a_class_without_subscription_is_not_found,
dlrl_8_1_6_1_2_1_a_relation_to_an_object_that_is_gone_is_not_found,
dlrl_8_1_6_1_2_2_updates_apply_transparently_until_disabled,
dlrl_8_1_6_1_2_3_one_round_reports_several_updates_and_the_modified_attributes}`;
`crates/dlrl/src/cache/mod.rs::tests::{dlrl_8_1_6_1_2_rejected_notes_are_bounded_and_count_what_they_drop,
dlrl_8_1_6_1_2_2_manual_mode_registers_the_last_update_of_each_instance}`;
green on the Linux test host.

**Status:** done

### §8.1.6.1.3 DLRL requested functions — publishing and subscribing applications

**Spec:** §8.1.6.1.3, p. 185 (PDF) — the application controls the mix of
incoming updates and its own modifications.

**Repo:** `crates/dlrl/src/cache/mod.rs::Cache::{disable_updates,
enable_updates, load}` (hold and take incoming updates),
`crates/dlrl/src/cache/access.rs::CacheAccess::{create_contract,
refresh, write}` (clones that take incoming updates only at `refresh`;
`refresh` discards what was not written). The rows an object has in DCPS
are, for `write`, those the Cache published and those its tables hold
for the object's owner key
(`crates/dlrl/src/cache/mod.rs::Cache::{rows_in_dcps, publish}`): an
object received from another participant, cloned and destroyed, and the
constituents its destruction clones, are disposed, and so is the removed
item of a collection of such an object. A row another participant wrote
is registered with the Cache's DataWriter before the `dispose`.

**Tests:** `crates/dlrl/tests/track_radar_e2e.rs::{dlrl_8_1_6_1_3_disable_updates_holds_incoming_updates_while_the_application_writes,
dlrl_8_1_6_1_3_a_write_access_is_not_changed_by_incoming_updates_until_refresh,
dlrl_8_1_6_1_3_destroying_a_received_object_reaches_a_third_cache,
dlrl_8_1_6_1_3_removing_an_item_of_a_received_object_reaches_a_third_cache}`;
green on the Linux test host.

**Status:** done

### §8.1.6.2 DLRL entities

**Spec:** §8.1.6.2, p. 185-188 (PDF, with Figure 8.4 and the table on
p. 187). The table names 19 entities (the text says 17): CacheFactory,
CacheBase, Cache, CacheAccess, CacheListener, Contract, ObjectHome,
ObjectListener, Selection, SelectionCriterion, FilterCriterion,
QueryCriterion, SelectionListener, ObjectRoot, Collection, List, Set,
StrMap, IntMap. P. 188 lists eight exceptions (DCPSError,
BadHomeDefinition, NotFound, AlreadyExisting, AlreadyDeleted,
PreconditionNotMet, NoSuchElement, SQLError); each carries the string
attribute `message`. Outside that list the text raises three more:
`ExpiredTimeOut` (`Cache::lock`, §8.1.6.3.4), `ReadOnlyMode`
(`CacheAccess::write` in the IDL of §8.2.1.2.1) and `DDS::BadParameter`
(`FooHome::create_selection` in the implied IDL of §8.2.1.2.2).

**Repo:** `crates/dlrl/src/metamodel.rs::DlrlEntityKind` (the 19 names in
the order of the table); `crates/dlrl/src/error.rs::DlrlException` with
the eight listed and the three further exceptions, each with `message`,
`spec_name()` and the repository id `IDL:DDS/<name>:1.0` (§8.2.1.2.1
declares them in `module DDS` without a `#pragma prefix`);
`CacheAccess::write` raises `ReadOnlyMode`
(`crates/dlrl/src/cache/access.rs`). The run-time types `CacheFactory`,
`CacheBase`, `Cache`, `CacheAccess`, `CacheListener`, `Contract`,
`ObjectHome`, `ObjectListener`, `ObjectRoot` in `crates/dlrl/src/cache/`.

**Tests:** `crates/dlrl/src/metamodel.rs::tests::dlrl_8_1_6_2_entity_table_has_the_nineteen_names_of_page_187`,
`crates/dlrl/src/error.rs::tests::{dlrl_8_1_6_2_every_exception_carries_a_message,
dlrl_8_1_6_2_repository_ids_are_in_module_dds}`,
`crates/dlrl/tests/cache.rs::dlrl_8_1_6_2_write_on_a_read_only_access_raises_read_only_mode`;
green on the Linux test host.

**Status:** done — the name table and the exceptions are complete. The
entities as types of their own are in the following item.

### §8.1.6.2 (rest) DLRL entity classes as standalone types

**Spec:** §8.1.6.2, p. 185-188 (PDF, Figure 8.4 and the table on
p. 187) — the 19 entities as classes with their attributes and
associations; the text on `Cache`: a Publisher and a Subscriber belong
to exactly one Cache, only objects of one Cache are put in relation.
With them the value types of the PSM (§8.2.1.2.1) the attributes refer
to.

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
UNLIMITED_RELATED_OBJECTS}`. An object is one memory cell per Cache or
CacheAccess (`ObjectRoot` is a shared handle). Each Cache creates its
own Publisher and Subscriber. `ObjectRoot::set_reference` and the
`*_object` operations of the collections refuse an object of another
Cache, and an object without oid (from `create_unregistered_object`,
before `register_object`), with `PreconditionNotMet`. Deviations: `CacheListener`,
`ObjectListener`, `FilterCriterion`, `SelectionListener` and
`Collection` are traits; `CacheBase` and `SelectionCriterion` are enums
over their subclasses; the `keys` of `IntMap` are a `LongSeq` as in the
IDL (the table of §8.1.6.3.19 says `string[]`).

**Tests:** `crates/dlrl/tests/entities.rs::{dlrl_8_1_6_2_figure_8_4_associations_navigate,
dlrl_8_1_3_1_two_references_to_one_oid_are_the_same_cell,
dlrl_8_1_6_2_a_publisher_and_a_subscriber_belong_to_one_cache,
dlrl_8_1_6_2_a_relation_across_two_caches_is_refused}`,
`crates/dlrl/src/types.rs::tests::dlrl_8_2_1_relation_description_kind_follows_its_subtype`,
`crates/dlrl/tests/cache.rs::dlrl_8_1_5_2_cache_usage_decides_publisher_subscriber_or_both`,
`crates/dlrl/tests/entity_tables.rs::dlrl_8_1_6_3_7_auto_deref_false_loads_the_state_on_access`
(the attribute `auto_deref` of `ObjectHome`, Figure 8.4);
green on the Linux test host.

**Status:** done — all 19 entities are types with their attributes and
associations, `ObjectHome::auto_deref` included (item §8.1.6.3 rest).

### §8.1.6.3 Details on DLRL entities

**Spec:** §8.1.6.3, p. 188-207 (PDF) — one table of attributes and
operations per entity, §8.1.6.3.1 to §8.1.6.3.19 (the text said 17).
The tables give only the functional return values; the PSM sets the
errors (p. 188). §8.1.6.4: the order of the listeners of an update
round.

**Repo:** per sub-section:
§8.1.6.3.1 CacheFactory, §8.1.6.3.2 CacheBase and §8.1.6.3.4 Cache in
`crates/dlrl/src/cache/mod.rs`; §8.1.6.3.3 CacheAccess and §8.1.6.3.6
Contract in `crates/dlrl/src/cache/access.rs`; §8.1.6.3.5
CacheListener in `crates/dlrl/src/cache/mod.rs`; §8.1.6.3.7
ObjectHome (with `create_selection`, `delete_selection`, `selections`,
`content_filter` after Annex B in `crates/dlrl/src/criterion.rs`) and
§8.1.6.3.8 ObjectListener (the three operations return `bool`; TRUE
from all listeners of one level ends the propagation to the homes of
the parent classes) in `crates/dlrl/src/cache/home.rs`; §8.1.6.3.9 to
§8.1.6.3.13 Selection, SelectionCriterion, FilterCriterion,
QueryCriterion, SelectionListener in
`crates/dlrl/src/cache/selection.rs`; §8.1.6.3.14 ObjectRoot with
`read_state` and `write_state` after Figures 8.5 and 8.6 (VOID, NEW,
NOT_MODIFIED, MODIFIED, DELETED) in `crates/dlrl/src/cache/object.rs`;
§8.1.6.3.15 to §8.1.6.3.19 Collection, List, Set, StrMap, IntMap with
`added_elements`, `removed_elements`, `modified_elements` of the last
update round or the last `refresh` in
`crates/dlrl/src/cache/collection.rs`; §8.1.6.4 in `Cache::notify`
(creation: ObjectListeners first, then Selections; modification and
deletion: Selections first, then ObjectListeners). The exceptions in
`crates/dlrl/src/error.rs::DlrlException`.
`ObjectListener::on_object_created` receives the `ObjectRoot` as in the
IDL of §8.2.1.2.2, not the `ObjectReference` of the PIM table.
`QueryCriterion::set_query` and `set_parameters` return TRUE; a failure
of the expression or the parameters is the `SQLError` of the IDL.
`delete_selection` deletes the Selection together with its criterion
(§8.1.6.3.7, p. 196): a deleted `QueryCriterion` raises `AlreadyDeleted`
from `set_query` and `set_parameters` and changes nothing (item §8.2.1),
and `create_selection` refuses it with `BadParameter`;
a `FilterCriterion` is an object of the application, and the Selection
releases its reference to it. For a clone in a CacheAccess,
`added_elements`, `removed_elements` and `modified_elements` are relative
to the previous `refresh`.
`disable_updates` in the middle of an update round (§8.1.6.3.4, p. 192)
interrupts it (`crates/dlrl/src/cache/mod.rs::{Cache::disable_updates,
Cache::last_round_interrupted}`): `last_round_interrupted` is TRUE when
`on_end_updates` is called and becomes FALSE again with the next round
with listeners. The text says the Listener receives `end_updates` "with
a parameter" that indicates the interruption; the table of §8.1.6.3.5
and the IDL of §8.2.1.2.1 give `on_end_updates` no parameter. Built is
the signature of table and IDL; the `CacheListener` reads the indication
at the Cache in `on_end_updates` (an extension). Called in
`on_begin_updates`, the updates of the round stay registered and no
ObjectListener hears them; later in the round they are applied already
(a round applies its updates before the first ObjectListener), and the
listeners hear the rest of the round.

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
(`register_object` with an existing key raises `AlreadyExisting`);
`crates/dlrl/src/error.rs::tests::dlrl_8_1_6_2_every_exception_carries_a_message`;
`crates/dlrl/tests/entity_tables.rs::{dlrl_8_1_6_3_4_disable_updates_in_on_begin_updates_interrupts_the_round,
dlrl_8_1_6_3_4_disable_updates_from_an_object_listener_interrupts_the_round}`;
green on the Linux test host.

**Status:** done — §8.1.6.3.1 to §8.1.6.3.19 and §8.1.6.4 are built,
also the interruption of a round by `disable_updates`, indicated by
`Cache::last_round_interrupted` instead of the parameter of
`end_updates` that table and IDL do not have; `lock`, `unlock` and the transformation of an `ObjectReference`
(§8.1.6.3.4), `auto_deref` with `set_auto_deref`, `deref_all` and
`underef_all` (§8.1.6.3.7) and `which_contained_modified` (§8.1.6.3.14)
are in the following item.

### §8.1.6.3 (rest) Method signatures of the entity tables

**Spec:** §8.1.6.3.4, p. 191-193 (PDF): `lock(to_in_milliseconds)` with
`ExpiredTimeOut`, `unlock`, the transformation of an `ObjectReference`
into an `ObjectRoot`; §8.1.6.3.7, p. 194-196: `auto_deref`,
`set_auto_deref`, `deref_all`, `underef_all`; §8.1.6.3.14, p. 200-201:
`which_contained_modified`.

**Repo:** `crates/dlrl/src/cache/mod.rs::{Cache::lock, Cache::unlock,
Cache::deref_reference, Gate}`: `lock` takes the same exclusion that
every update round, `load`, `refresh`, `enable_updates` and
`Selection::refresh` take; other threads and the rounds of the Cache's
thread wait, samples stay in the DataReaders and form the next round.
`ExpiredTimeOut` when the time-out runs out; the holding thread may lock
again and unlocks as often. Deviations where the text says nothing: a
negative time-out is `BadParameter`, `unlock` by a thread without the
lock is `PreconditionNotMet`. `delete_cache` closes the lock before it
ends the Cache's thread: a closed lock is given to nobody, and whoever
waits for it stops waiting. `delete_cache` therefore returns also while
the calling thread or another thread holds the lock; a waiting `lock`
raises `AlreadyDeleted`, `unlock` gives the lock back and then raises
`AlreadyDeleted`. When a `CacheListener` calls `delete_cache` in
`on_begin_updates` on the Cache's thread, it returns; the round applies
nothing more, calls no other listener but `on_end_updates` and ends
without a panic. When an `ObjectListener` or a `SelectionListener`
calls it, the round calls no further listener but `on_end_updates`:
not the next listener of the same home, not those of the parents'
homes, and no further Selection is examined. `delete_cache` discards the registered updates, and
`load`, `enable_updates` and the rounds check after each lock of the
state whether the Cache is deleted. `deref_reference` is the unnamed
transformation (p. 193): the Cache's `ObjectRoot` with its state loaded,
`NotFound` when there is no such object.
`crates/dlrl/src/cache/home.rs::{ObjectHome::auto_deref, set_auto_deref,
deref_all, underef_all}`: with FALSE an arriving object holds only class
and oid, its state stays in the rows of the Cache's DCPS tables and is
loaded on the application's access (`get`, `data`, `get_reference`,
`get_references`, a collection's contents, `deref_reference`);
`ObjectRoot::is_dereferenced` shows it (an extension). The update round
rebuilds the state before the round from the rows to find the modified
attributes; read_state, `is_<attribute>_modified`, listeners, Selections
(a `QueryCriterion` does not load) and the clones of a CacheAccess work
as with TRUE; a deleted object keeps its last state.
`set_auto_deref(TRUE)` loads all objects of the home; `auto_deref`
concerns the objects of the home's class, derived classes have their own
home. `crates/dlrl/src/cache/object.rs::ObjectRoot::which_contained_modified`:
per composition relation with a contained object for which
`is_modified(CONTAINED_OBJECTS_SCOPE)` holds, the name and for a `List`
the index, for an `IntMap` or `StrMap` the key; a `Set` relation once as
`RelationDescription::Set` with the extension `RelationKind::SetRelation`
(`crates/dlrl/src/types.rs`), since the PSM's `RelationKind` has no value
for a set.

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
green on the Linux test host.

**Status:** done — the operations of §8.1.6.3.4, §8.1.6.3.7 and
§8.1.6.3.14 that item §8.1.6.3 lacked are built.

## §8.2 OMG IDL Platform Specific Model (PSM)

### §8.2.1 Run-time entities

**Spec:** §8.2.1.1, pp. 211-212 (PDF), the mapping rules: entities that
are access points to servicing objects (e.g. Cache) are local
interfaces, application objects and their parts are valuetypes; an error
that affects the future behaviour is an exception, a warning without
effect (e.g. removing from a list what is not in it) is a return value;
`get_instance` of CacheFactory is a static operation of the language
and not in the IDL; native types appear as typedefs with a `#define`
with the suffix `_TYPE_NATIVE`; `AlreadyDeleted` can be raised by every
operation and attribute and is therefore in no raises clause. §8.1.6.2,
p. 188: `AlreadyDeleted` when an operation is invoked on an object that
has already been deleted. `DLRL_initialize` does not occur in
formal/07-01-01; the entry point is `CacheFactory::get_instance`.

**Repo:** `crates/dlrl/idl/dds_dlrl.idl` (the kinds of §8.2.1.1, the IDL
itself in the following item); `crates/dlrl/src/cache/mod.rs::CacheFactory::get_instance`
(static, the same factory at every call); `crates/dlrl/src/types.rs::DlrlOid`
(three `i32` for `DLRL_OID_TYPE_NATIVE long`). A warning as a return
value (`crates/dlrl/src/cache/collection.rs`): `remove` of `List`, `Set`,
`StrMap`, `IntMap` and `Set::remove_object` return FALSE when there was
nothing to remove; the IDL declares these `remove` operations `boolean`
(change 9 of `dds_dlrl.idl`, `FooSet::remove` in the implied IDL; the
print has `void`). Before that, every writing operation of a collection
checks the object as `ObjectRoot::set` does: `AlreadyDeleted` for a
destroyed object, `PreconditionNotMet` for an object that is not in a
CacheAccess that allows writing (an object of the Cache); so does
`Set::add` of an element already there. `get` of a missing element
raises `NoSuchElement`. `AlreadyDeleted`
(`crates/dlrl/src/cache/{mod.rs, access.rs, selection.rs, home.rs,
object.rs, collection.rs}`): every operation of a deleted Cache
(`delete_cache`, which also deletes its CacheAccesses), CacheAccess
(`delete_access`), Contract (`delete_contract`, `purge`, deletion of the
CacheAccess), Selection (`delete_selection`; also `refresh` of a
Selection of a deleted Cache) and its `QueryCriterion` raises it, as do a
second deletion, `create_object`, `create_unregistered_object` and
`register_object` with a deleted CacheAccess, and `find_object`,
`get_objects`, `get_created_objects`, `get_modified_objects` and
`get_deleted_objects` with a deleted Cache or CacheAccess as source. A
destroyed object (write_state DELETED: after `destroy`, also after the
`write` that published the destruction, and after `purge` or deletion of
a CacheAccess that allows writing) raises it from every operation:
`destroy`, `is_modified`, `which_contained_modified`, the generated
`get_<attribute>`, `set_<attribute>` and `is_<attribute>_modified`
(`get`, `set`, `is_attribute_modified`), the navigation, `get_list`,
`get_set`, `get_str_map`, `get_int_map` and every operation of its
collections, also through handles taken before. `Cache::attach_listener`,
`detach_listener`, `enable_updates`, `disable_updates`,
`find_home_by_name`, `find_home_by_index`, `CacheAccess::purge`,
`Contract::set_depth`, `set_scope`, `Selection::set_listener`,
`ObjectRoot::is_modified`, `which_contained_modified`,
`is_attribute_modified`, `Set::contains`, `added_elements`,
`removed_elements` and `modified_elements` of the collections and the
four listing operations of `ObjectHome` return a `Result` for it. The
attributes of a deleted entity and of a destroyed object keep their last
value, also `length`, `values` and `keys` of a collection; §8.2.1.1 names
attributes as places where `AlreadyDeleted` can be raised, §8.1.6.2
requires it for operations. An object with read_state DELETED (deleted
by another participant, or no longer contracted at a `refresh`) is not
destroyed: the listeners of a deletion and `get_deleted_objects` give it
to the application, and its operations read its last state (§8.1.6.4).
The repository ids `IDL:DDS/<Name>:1.0` in `crates/dlrl/src/error.rs`.

**Tests:** `crates/dlrl/tests/psm.rs::{dlrl_8_2_1_1_access_points_are_local_interfaces_and_objects_are_valuetypes,
dlrl_8_2_1_1_get_instance_is_not_in_the_idl_and_exists_in_rust,
dlrl_8_2_1_1_a_warning_is_a_return_value,
dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_deleted_entity,
dlrl_8_2_1_1_already_deleted_from_every_operation_of_a_destroyed_object,
dlrl_8_2_1_1_remove_checks_the_object_before_the_warning}`
(calls, on a deleted Cache, CacheAccess, Contract, Selection and
QueryCriterion, every operation the IDL gives the interface with its
bases, and requires the operations called to be exactly those of the
IDL; calls, on a destroyed object and its collections, every operation
the IDL gives the valuetype and the generated operations, before and
after the `write`),
`crates/dlrl/tests/selections.rs::dlrl_8_1_6_3_7_create_and_delete_selection_check_their_preconditions`,
`crates/dlrl/src/types.rs::tests::dlrl_8_2_1_relation_description_kind_follows_its_subtype`,
`crates/dlrl/src/error.rs::tests::dlrl_8_1_6_2_repository_ids_are_in_module_dds`;
green on the Linux test host.

**Status:** done — the rules of §8.2.1.1 hold for the Rust API and the
IDL; every operation of a deleted entity and of a destroyed object
raises `AlreadyDeleted`; the attributes keep their last value.

### §8.2.1 (rest) IDL PSM emission + DLRL_initialize

**Spec:** §8.2.1.2, pp. 212-228 (PDF): §8.2.1.2.1 the IDL of the generic
entities, §8.2.1.2.2 the implied IDL of each application class `Foo`:
`FooSeq`, `FooListener`, `FooListenerSeq`, `FooSelectionListener`,
`FooFilter`, `FooQuery`, `FooSelection`, `FooSelectionSeq`, `FooHome`,
`FooList`, `FooSet`, `FooStrMap`, `FooIntMap`. `DLRL_initialize` does
not exist in the specification (see item §8.2.1). The printed IDL has
defects a compiler finds: `contracted_object.` for `;`, `ReadOnlyMode`
and `DDS::BadParameter` raised and not declared, `CacheBase` unscoped in
`FooHome`, and `FooListener` and `FooSelectionListener` declare the
base's `on_object_created`, `on_object_deleted` and `on_object_out`
again, which IDL 4.2 §7.4.3 forbids.

**Repo:** `crates/dlrl/idl/dds_dlrl.idl`: the declarations of §8.2.1.2.1
in its own layout; its header names nine changes against the print (the
semicolon; `ReadOnlyMode`, `ExpiredTimeOut`, `BadParameter` declared; the
three listener operations in the comment blocks of the operations
generated typed, where `on_object_modified` and `on_object_in` already
stand; `create_contract` returns the Contract as the table of §8.1.6.3.3
says; `Cache::load`, `lock`, `unlock` of the table of §8.1.6.3.4
declared; `delete_cache` raises `PreconditionNotMet` and `DCPSError`;
the parameter `object` written `_object`, since `object` differs from
the keyword `Object` only in case; `CacheDescription` a struct, since a
valuetype cannot hold the local `DomainParticipant`; `remove` of `List`,
`StrMap` and `IntMap` returns `boolean`, the warning of §8.2.1.1, for
which `void` has no place).
`crates/dlrl/idl/dds_dcps.idl`: `StringSeq` and the entity interfaces
`DomainParticipant`, `Publisher`, `Subscriber` without operations.
`crates/dlrl/tests/psm.rs::binding` binds each declaration (type,
enumerator, member, attribute, operation, exception, constant) to its
Rust item, so that a missing item breaks the build.
`crates/dlrl-codegen/src/idl.rs::generate_implied_idl`: the thirteen
constructs for one class in its modules, with `DDS::CacheBase` scoped,
the operations of `FooListener` returning `boolean` as the table and the
generic IDL say, `FooHome::create_selection` with
`concerns_contained_objects` and `PreconditionNotMet`, `find_object`
with `NotFound`, `FooSet::remove` with `boolean` like the `remove`
operations of the generic IDL. CLI: `zerodds-idlc dlrl --lang idl`
(`tools/idlc/src/dlrl.rs::{application_classes, implied_idl_file}`)
finds the valuetypes that derive, directly or through other valuetypes,
from `DDS::ObjectRoot` and writes `<stem>_dlrl.idl`, which includes the
model description and declares the implied IDL of each class;
`--lang all` includes it.

**Tests:** `crates/dlrl/tests/psm.rs::{dlrl_8_2_1_2_1_generic_idl_parses,
dlrl_8_2_1_2_1_every_idl_declaration_has_its_rust_item}`;
`crates/dlrl-codegen/src/idl.rs::tests::dlrl_8_2_1_2_implied_idl_declares_every_derived_construct`,
`crates/dlrl-codegen/tests/implied_idl.rs::{dlrl_8_2_1_2_implied_idl_for_track_track3d_radar_parses,
dlrl_8_2_1_2_the_printed_listener_redefinition_is_refused}`;
`tools/idlc/tests/dlrl.rs::{dlrl_8_2_1_2_lang_idl_writes_the_implied_idl_of_track_track3d_radar,
dlrl_8_2_1_2_lang_idl_and_all_write_what_the_file_has_input_for}`,
`tools/idlc/src/main.rs::tests::dlrl_8_2_1_2_lang_idl_asks_for_the_implied_idl_alone`;
green on the Linux test host. With a foreign IDL compiler:
`crates/dlrl-codegen/tests/omniidl.rs::{dlrl_8_2_1_2_omniidl_compiles_generic_and_implied_idl,
dlrl_8_2_1_2_omniidl_refuses_the_printed_listener_redefinition}`
(omniidl 4.3.3 with its C++ back end; in the weekly job
`integration-bridges-vendor-corba`, scenario `dlrl-idl-omniidl`), green
on the Linux test host.

**Status:** done — the generic IDL is a file and each declaration has
its Rust item; the implied IDL is generated for each application class,
by the library and by `zerodds-idlc dlrl --lang idl`, and compiled by the
IDL frontend and by omniidl.

### §8.2.2 Generation process

**Spec:** §8.2.2, pp. 228-234 (PDF). §8.2.2.1 (Figure 8.7): from the
model description (IDL valuetypes) and the model tags (XML) the DLRL
generator produces the enhanced model description with implied IDL, the
DLRL entities as IDL and the DCPS description; the language mappings
turn them into native constructs. §8.2.2.2: application classes are
valuetypes, a typed collection is a forward valuetype declaration that a
`templateDef` defines. §8.2.2.3: the model tags follow the DTD of
§8.2.2.3.1 (encoding ISO-8859-1); §8.2.2.3.2.1 to .14 explain `Dlrl`,
`enumDef`, `templateDef`, `associationDef`, `compoRelationDef`,
`classMapping`, `mainTopic`, `keyDescription`, `extensionTopic`,
`monoAttribute`, `multiAttribute`, `monoRelation`, `multiRelation`,
`local`. The example snippets of §8.2.2.3.2 are not well-formed XML (a
missing `>`, `<\multiPlaceTopic>`, `class"Radar"`, the contents
`SimpleOID`, `FullOID`, `FullSimpleOID` against the DTD); the tests take
well-formed versions. The word "pragma" does not occur in
formal/07-01-01.

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree_bytes` (reads an XML
document from bytes in UTF-8 or ISO-8859-1 as its declaration says, with
a DOCTYPE line; `parse_xml_tree` keeps the UTF-8 rule of DDS-XML
§7.1.1); `crates/dlrl/src/model_tags.rs::{parse_model_tags, ModelTags}`
(feature `model-tags`: the DTD of §8.2.2.3.1 checked in full — elements,
the order `mainTopic?, extensionTopic?`, required and allowed
attributes, the enumerated values of `pattern` and `content`, the number
of `keyField` per `content`, text only in `value`, `keyField`,
`valueField`; each deviation an error with the path of the element);
`crates/dlrl/src/generation.rs::{ModelDescription, generate}` (the
generator: valuetypes become classes, a basic type, an enumeration or a
simple structure a mono-valued attribute, a valuetype of the description
a to-1 relation, a forward declaration a `templateDef` defines a to-many
relation or a multi-valued attribute; `local`, `compoRelationDef`,
`associationDef` and `enumDef` applied; the `classMapping` elements with
the default mapping of §8.1.4.3 for everything they leave out; names
that do not resolve are errors, also the `itemType` of a `templateDef`
no member uses; a `templateDef` or `enumDef` no member uses is an error;
a `templateDef` or `enumDef` name that
differs only in case is taken, as §8.2.3 writes `stringStrMap` and
`StringStrMap`).

Beside it the older pragma dialect (not from the specification):
`crates/dlrl/src/pragma.rs::{DlrlPragma, parse_pragma}` reads
`#pragma DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION`;
`crates/dlrl-codegen/src/{cpp,csharp,java,ts}.rs` generate an object
class and a home per type from them; CLI `tools/idlc/src/dlrl.rs`
(`zerodds-idlc dlrl --lang {cpp,csharp,java,typescript}`), independent of
the AST path `idl-lower` reads `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` through
for OpenDDS's dialect (`crates/idl-lower/src/lower.rs`, whose diagnostic
names both readings).

**Tests:** model tags and generator:
`crates/dlrl/tests/model_tags.rs::{dlrl_8_2_2_3_the_full_tags_of_8_2_3_3_parse,
dlrl_8_2_2_1_the_example_generates_model_and_mapping,
dlrl_8_2_2_3_the_minimal_tags_of_8_2_3_3_take_the_default_mapping,
dlrl_8_2_2_3_1_each_violation_of_the_dtd_is_refused_with_its_path,
dlrl_8_2_2_1_tags_apply_to_the_description_and_unresolved_names_are_refused}`,
`crates/xml/src/parser.rs::tests::dlrl_8_2_2_3_1_iso_8859_1_document_read_from_bytes`;
green on the Linux test host.

Pragma dialect — pragma parser:
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
attribute of the object class (§8.1.3.1, §8.1.4.3). Before, the
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

**Status:** partial — built and green on the Linux test host: reading
the model tags against the DTD of §8.2.2.3.1 and the generator that
turns model description and model tags into model and mapping. Open:
(1) `zerodds-idlc` makes model and mapping from the IDL valuetypes of
the model description and the model tags (`--tags`); today
`dlrl --lang idl` finds the valuetypes with base `DDS::ObjectRoot` only
for the implied IDL (item §8.2.1 rest), the four languages read the
pragma dialect, and `DLRL::ObjectRoot` and `#include "dlrl.idl"` as in
§8.2.3.2 are not recognised. (2) Of the outputs of Figure 8.7 the DCPS
description as IDL is missing; `dlrl --lang idl` writes the implied IDL,
the DLRL entities as IDL are the file `crates/dlrl/idl/dds_dlrl.idl`. (3) The generated classes
of §8.1.6.6 (`FooHome` with `create_object(access)`, `FooListener`,
`FooSelection`, `FooSelectionListener`, `FooFilter`, `FooQuery`, typed
collections) for Rust and the four languages; today's classes have a
`delete_object` the specification does not have, and derive from
`DLRL::ObjectRoot`, for which no runtime exists in C++, Java, C# and
TypeScript (`tools/idlc/tests/dlrl_compiles.rs` compiles against stubs).
(4) A runtime for C++, Java, C# and TypeScript through
`crates/zerodds-c-api`.

### §8.2.3 Example

**Spec:** §8.2.3, p. 234-244 (PDF) — illustrative example code.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — example.

## Annex B

### Annex B: Syntax for DLRL Queries and Filters

**Spec:** Annex B, pp. 245-246 (PDF) — B.1 the grammar (Condition with
AND, OR, NOT and parentheses; ComparisonPredicate in three forms;
BetweenPredicate with BETWEEN and NOT BETWEEN over a Range of two
Parameters; the RelOps `=`, `>`, `>=`, `<`, `<=`, `<>`; Parameter),
B.2 the tokens FIELDNAME (`.` into structures, `[INTEGERVALUE|STRING]`
into collections), INTEGERVALUE (with sign and `0x`), CHARVALUE,
FLOATVALUE (with exponent), STRING (starts with a left or right quote,
ends with a right quote), ENUMERATEDVALUE, PARAMETER `%n` with n < 100.
Annex B names the filter of the FilterCriterion and the query of the
QueryCriterion as its users; `check_object` of the FilterCriterion is
application code, though (§8.1.6.3.11). The two users of the grammar
are `QueryCriterion` (§8.1.6.3.12) and the `content_filter` of
`ObjectHome` (§8.1.6.3.7, p. 196). `SQLError` for bad syntax, an
unknown field or parameters that do not fit (§8.1.6.2).

**Repo:** `crates/sql-filter/src/parser.rs::{Grammar::Dlrl12, parse_with}`
(the grammar of Annex B: LIKE, `!=`, boolean literals, `null` and
`#discriminator` refused, a comparison needs a FIELDNAME, the Range of
BETWEEN is two Parameters, `%0` to `%99`; a STRING also opened by
`` ` ``, without `''` and without a new-line, the same in the subscript
`[STRING]`; a subscript in `[ ]` with sign and `0x`);
`crates/dlrl/src/criterion.rs::QueryExpression` (expression and
parameters checked against the class: every FIELDNAME leads through
attributes, structures, relations, `List`, `IntMap`, `StrMap` and
`sequence` to a basic type or an enumeration; constants and parameters
fit their field, an enumeration compares by the position of its
enumerators; the number of parameters is the one required; every
parameter is a Parameter token, also one whose `%n` does not occur;
`set_parameters`; evaluation in three-valued logic, a missing value
makes the predicate unknown; `QueryGrammar::Dcps14` as a named option
with LIKE, there too `%n` only with n < 100 as in Annex B of DDS 1.4, a
larger n is `SQLError`); `crates/dlrl/src/cache/home.rs::{ObjectHome::set_content_filter,
ObjectHome::content_filter}` applied in the update round
(`crates/dlrl/src/cache/mod.rs`); a FIELDNAME of the `content_filter`
that goes across a relation is `SQLError`
(`QueryExpression::fields_across_relations`). The older subset query.rs (ordering,
limit, closure filter; not in Annex B) is removed.

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
green on the Linux test host.

**Status:** done — grammar and tokens complete, checked against the
class and evaluated; `content_filter` uses them. The `content_filter` is
applied in the Cache's update round to the object built from all its
rows, not as a ContentFilteredTopic of DCPS, because the attributes of
one object may lie in several topics. A FIELDNAME of the
`content_filter` does not go across a relation: the filter is to be
mapped on DCPS (§8.1.6.3.7, "intended to be mapped on the underlying
DCPS infrastructure") and decides on the data of the incoming object; a
field of another object would make the decision depend on that object's
arrival. `QueryCriterion` as a type with
`set_query` over `QueryExpression` is part of the item §8.1.6.2 (rest).

---

## Audit status

17 done / 6 partial / 0 open / 0 blocked / 4 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document dlrl-1.2.md`):

* `cargo nextest run --profile merge -p zerodds-dlrl --all-targets` (job `merge-rust-dlrl`) — 64 tests green.
* `cargo nextest run --profile merge -p zerodds-dlrl-codegen --all-targets` (job `merge-rust-dlrl-codegen`) — 9 tests green.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (job `merge-rust-idl-lower`) — 66 tests green.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 20 tests green.
* `python3 ci/run_integration.py integration-codegen-csharp` (job `integration-codegen-csharp`) — 8 tests green.

167 cited tests green, 0 not green.

Open items: `dlrl-1.2.open.en.md`.
