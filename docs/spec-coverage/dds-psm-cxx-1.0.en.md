# DDS-PSM-Cxx 1.0 (ISO/IEC C++ 2003 Language DDS PSM) — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-psm-cxx-1.0.pdf` (34 pages, OMG
formal/2013-11-01, November 2013) —
[OMG DDS-PSM-Cxx 1.0](https://www.omg.org/spec/DDS-PSM-Cxx/1.0/)

Item-by-item audit against the PDF following
`internal/spec-coverage/PROCESS.md`. Page numbers without a further note are
PDF pages of this PDF (PDF page = printed page + 10). Every normative sentence
of §1 to §8, every row of Table 7.1, Table 7.2 and Table 7.3 and of the tables
of §7.4.5 and §7.4.6, and every operation of the classes the PDF writes out
(`safe_enum`, `SampleLostStatus`, `THistory`, `policy_id`, `policy_name`,
`TQosProvider`) is an item of its own. §2.1 requires all programming
interfaces of the conformance levels of [DDS]; by §3 [DDS] is version 1.2
(`internal/standards/cache/omg/dds-1.2.pdf`, PDF page = printed page + 12).
The operations of the DCPS classes ([DDS] 1.2 §7.1.2, p. 25–107) and the
built-in topics (§7.1.5, p. 147–150) are therefore one item each under §2.1;
§7.5.4 requires the status classes of [DDS] (IDL p. 159–161) and §7.6.1 the
policies ([DDS] 1.2 §7.1.3, p. 108–120), one item per class. Operations that
differ only by `_w_timestamp`, and pairs of `set_` and `get_`, are one item.
`done` is used only where code and a test cover the requirement; a cited test
names the test function as `file::module::name`, a C++ test as
`file::function`. A repository reference names the file and the symbol; it
carries no line number. This document names tests; their results come from the
Linux test host, not from this document. Where code implements a requirement
and no test checks all of it, the item is `partial` and the note says what is
not checked.

**Context:** DDS-PSM-Cxx 1.0 is spread over these parts of ZeroDDS:

- `crates/cpp/` (`zerodds-cpp`) — the hand-written headers under
  `crates/cpp/include/dds/` (`dds::core`, `dds::domain`, `dds::topic`,
  `dds::pub`, `dds::sub`), the CMake target `zerodds-cpp` in
  `crates/cpp/CMakeLists.txt` (headers only; C++03 and later) and the tests:
  `crates/cpp/tests/smoke_dds_psm.cpp` (compiled and run by
  `crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke`, in CMake the
  target `smoke_dds_psm`), `crates/cpp/tests/generated_type_roundtrip.cpp` and
  `crates/cpp/tests/generated_psm_roundtrip.cpp` (compiled and run by
  `crates/cpp/tests/generated_type_via_cargo.rs`, the second with
  `--cpp-mapping psm`).
- `crates/zerodds-c-api/` (`libzerodds`, `include/zerodds.h`) — every
  operation of the headers calls a function of the C API; the C++ classes hold
  only pointers to the objects of the C API as their state.
- The C++ code generator: `zerodds-idlc generate --lang c++`
  (`tools/idlc/src/generate.rs`) through `crates/lang-rules` (`adapter/cpp.rs`,
  `emit/cpp.rs`) and `crates/lang-render/src/cpp.rs`. It generates the topic
  types and, per type, the specialization `dds::topic::topic_type_support<T>`
  that `Topic<T>`, `DataWriter<T>` and `DataReader<T>` use.

`crates/cpp/include/zerodds/dds.hpp` (`zerodds::Runtime`,
`zerodds::TypedWriter<T>`) is a separate API outside `dds::` and is not covered
by this document. The mapping of IDL to C++ by IDL4 to C++ is in
[`idl4-cpp-1.0.en.md`](idl4-cpp-1.0.en.md), the endpoint API in
[`zerodds-endpoint-cpp-1.0.en.md`](zerodds-endpoint-cpp-1.0.en.md); this
document covers only the rules DDS-PSM-Cxx itself sets for the types (§7.4,
§8).

By §2 the PDF and the OMG C++ headers are both normative, and the headers
prevail in a conflict. The headers
(`http://www.omg.org/spec/DDS-PSM-Cxx/20121110/sources`, 196 files) are in the
cache under `internal/standards/cache/omg/dds-psm-cxx-1.0-sources/`; the API
documentation is not. Compared with a script are the file names and the
identifiers (classes, typedefs, functions, macros) of all headers outside
`detail/`; not compared are `const`, return types and exceptions of the
individual signatures. Where PDF and headers differ, an item names the
difference and says which one ZeroDDS follows. An item under §2.1 checks whether
the C++ API offers the operation under the name of the headers.

**Open items:** none; every item is `done` or `n/a (informative)`.

---

## §1 Scope

### §1 Purpose: a new C++ API for DDS

**Spec:** §1, p. 11 — "The purpose of this document is to specify the ISO/IEC
C++ PSM for DDS. This new PSM provides a new C++ API for programming DDS which
is clear, simple, expressive, safe, efficient, extensible, and portable."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement of purpose; the requirements start at §2.

### §1 No effect on the wire, API from headers

**Spec:** §1, p. 11 — "The ISO/IEC-C++ PSM does not impact on-the-wire
interoperability with other language mappings. The PSM API is defined by means
of a set of C++ header files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement about the specification. The ZeroDDS headers
have no path to the wire of their own: they call the C API. The bytes of a sample come
from `topic_type_support<T>::encode` (generated by the code generator) or, for
`dds::core::ByteSeq` and `std::string`, from the specialization in
`crates/cpp/include/dds/topic/TopicTraits.hpp`, which passes the bytes on unchanged.

### §1 Scope: all DCPS profiles, XTypes, QoS profiles, no DLRL

**Spec:** §1, p. 11 — "This PSM includes all DCPS conformance profiles defined
in the DDS specification. In addition, it includes platform-specific mappings
for: • The programming interface specified by [DDS-XTypes] • Accessing QoS
profiles such as are specified in [DDS-CCM] This specification only addresses
the DCPS layer of the DDS specification. The optional DLRL layer may be
addressed separately in a future specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Scope; the requirements for it are in §2.1 (DCPS
profiles, the Extensible and Dynamic Types level, XML QoS profiles).

### §1 New C++ mapping of the type system

**Spec:** §1, p. 11 — "This specification also introduces a new C++ mapping for
the DDS type system as specified in the Extensible and Dynamic Topic Types
Specification [REF]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Pointer to §8. The PDF writes "[REF]" instead of a
reference.

---

## §2 Conformance

### §2 Document and C++ headers normative, the headers prevail

**Spec:** §2, p. 11; Titelseite, p. 1 — "This specification consists of this
document as well as a set of C++ header files, references on the cover page.
Both are normative. In the event of a conflict between them, the latter shall
prevail."

**Repo:** `crates/cpp/include/dds/dds.hpp`, `crates/cpp/CMakeLists.txt`

**Tests:** `crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own`,
`crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_signatures_follow_the_normative_headers`

**Status:** `done` — The OMG headers are in the cache
(`internal/standards/cache/omg/dds-psm-cxx-1.0-sources/`, 196 files). Compared are file
names, identifiers (classes, typedefs, functions, the macros of `refmacros.hpp`) and the
signatures of the members (name, number of parameters, `const`, return type, by value or by
reference; `tools/psm-signature-diff.py`). Where they differ the headers prevail: the
getters of policies and statuses return values as in the OMG headers,
`on_offered_incompatible_qos` takes the writer by value. The spellings of the OMG headers
that differ from the DDS names (`offered_deadlined_missed_status`,
`offered_incompatible_qos_staus`, `matched_publications_data`) exist as second names. Every
public header compiles on its own (144 headers). Not compared: exception specifications and
default arguments; the differences the tool still shows are macro and spelling artefacts
of the textual comparison (read, not a finding). The CMake target
`zerodds_psm_standalone_headers` (test `the_cmake_target_for_standalone_headers_builds`) is
`#[ignore]` and did not run in this round.

---

## §2.1 Conformance Profiles

### §2.1 Conformance parallels the levels of [DDS]

**Spec:** §2.1, p. 11; [DDS] 1.2 §2, p. 14–15 (`dds-1.2.pdf`) — "Conformance to
this specification parallels conformance to the DDS specification itself and
consists of the same conformance levels. For example, an implementation may
conform to the DDS Minimum Profile with respect to this PSM, meaning that all of
the programming interfaces identified by the DDS specification as pertaining to
that conformance level must be implemented in this PSM."

**Repo:** `crates/cpp/include/dds/core/detail/conformance.hpp`,
`crates/cpp/include/dds/dds.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — All operations of the DCPS classes exist in the headers; the items of the
following sections give code and test per operation. `core/detail/conformance.hpp`
defines the switches of the OMG for content subscription, ownership, persistence and the
XTypes levels. ZeroDDS names no conformance level for the C++ API.

### §2.1 Object Model Profile (DLRL) excluded

**Spec:** §2.1, p. 11 — "The one exception to this rule is the Object Model
Profile, which defines the Data Local Reconstruction Layer (DLRL); DLRL is
outside of the scope of this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Delimitation by the specification. The Object Model
Profile includes `access_scope = GROUP` of PRESENTATION ([DDS] 1.2 §2, p. 15); the C++
API knows the value (`PresentationAccessScopeKind::GROUP` in
`crates/cpp/include/dds/core/policy/PolicyKind.hpp`).

### §2.1 Extensible and Dynamic Types conformance level

**Spec:** §2.1, p. 11 — "In addition to the conformance level defined in the DDS
specification itself, this PSM recognizes and implements the Extensible and
Dynamic Types conformance level for DDS defined by the Extensible and Dynamic
Topic Types for DDS specification."

**Repo:** `crates/cpp/include/dds/core/xtypes/TDynamicType.hpp`,
`crates/cpp/include/dds/core/xtypes/TStructType.hpp`,
`crates/cpp/include/dds/core/xtypes/UnionType.hpp`,
`crates/cpp/include/dds/core/xtypes/TDynamicData.hpp`,
`crates/cpp/include/dds/core/xtypes/TypeProvider.hpp`,
`crates/cpp/include/dds/core/policy/CorePolicy.hpp::DataRepresentation`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_dynamic_types_dynamic_data_and_type_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_bit_masks_union_defaults_and_enum_discriminators`,
`crates/zerodds-c-api/tests/types_load.rs::bitmasks_unions_with_a_default_and_enum_discriminators_are_described`

**Status:** `done` — `dds::core::xtypes` exists (commits `1c10c2b3b74`, `6e1c3aebfb8`): DynamicType, the primitive, string, sequence, map, array, structure, union, enumeration, alias, bit set and bit mask types, members, annotations, DynamicData and TypeProvider; with them the policy classes `DataRepresentation` and `TypeConsistencyEnforcement` and the built-in topic types. Details and `fixed` in the item "§7.11".

### §2.1 Creation and QoS from XML QoS profiles, else UNSUPPORTED

**Spec:** §2.1, p. 11 — "This PSM furthermore defines methods to create Entities
and to set their QoS based on the XML QoS libraries and profiles defined by the
DDS for Lightweight CCM specification. Implementations that support these XML
QoS profiles shall implement these operations fully; other implementations shall
indicate failure with the DDS-standard UNSUPPORTED error."

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp`,
`crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `dds::core::QosProvider` reads the QoS profiles of a DDS-XML
document from a `file:`, `data:`, `http:` or `https:` URI (commits `5061c168b7f`,
`9629b1b5a34`). A scheme without a client is `UnsupportedError` (`gopher://…` in
`test_qos_provider`). The QoS create entities (a DataWriter made with the provider's QoS
in `test_qos_provider`).

### §2.1 Plain Language Binding as an optional conformance point

**Spec:** §2.1, p. 11 — "The Plain Language Binding for C++ defined in this
specification represents an optional conformance point. Implementers may support
either this Language Binding or the previously defined Plain Language Binding
for C++ defined in [DDS-XTypes]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission to choose. By default (`--cpp-mapping psm`) the
code generator produces the binding of §8 (`zerodds-idlc`, commit `56ff5b5f25f`; items
on §8); the binding of [DDS-XTypes] 1.0 Beta 1 (ptc/2010-05-12) is not in the cache and
was not compared.

---

## §2.1 Programming interfaces of [DDS] 1.2: Entity

### §2.1 Entity: set_qos and get_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 25–26 (`dds-1.2.pdf`), table
Entity — "abstract set_qos (qos_list: QosPolicy []): ReturnCode_t; abstract
get_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::qos`,
`crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::qos`,
`crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::qos`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::qos`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::qos`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`

**Status:** `done` — `qos()` reads and `qos(const Qos&)` sets the QoS of
DomainParticipant, Publisher, Subscriber, Topic, DataWriter and DataReader; also `<<`
and `>>` on Publisher, Subscriber, DataWriter and DataReader. A changeable policy
changes, an immutable one is `ImmutablePolicyError`
(`test_qos_of_a_running_entity_is_set_and_read`).

### §2.1 Entity: set_listener and get_listener, listener at creation

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity
— "abstract set_listener (a_listener: Listener, mask: StatusKind []):
ReturnCode_t; abstract get_listener: Listener"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::listener`,
`crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::listener`,
`crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::listener`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::listener`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::listener`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::listener`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_attach_detach`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_domain_participant_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `listener(l, mask)` sets and `listener()` reads the listener of
each entity kind; every constructor takes a listener and a mask
(`test_entities_are_created_with_qos_listener_and_mask`). The table of the C API is per
entity and is deregistered on destruction; each callback of the writer, reader and
participant tables reaches its virtual method with the triggering entity
(`test_every_listener_callback_reaches_its_virtual_method`).

### §2.1 Entity: enable

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity
— "enable: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::enable`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `enable()` of every entity kind calls the `zerodds_*_enable`
functions of the C API; `test_entity_operations` calls it for participant, topic,
publisher, subscriber, DataWriter (twice) and DataReader.

### §2.1 Entity: get_statuscondition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity
— "get_statuscondition: StatusCondition"

**Repo:** `crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — The PSM has no operation `get_statuscondition`:
`StatusCondition(entity)` creates the entity's condition (`TStatusCondition`,
constructor from an entity), `entity()` returns it, `enabled_statuses` sets the mask.

### §2.1 Entity: get_status_changes

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity
— "get_status_changes: StatusKind []"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::status_changes`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `status_changes()` returns the triggered statuses;
`test_entity_operations` checks PUBLICATION_MATCHED, SUBSCRIPTION_MATCHED and
DATA_AVAILABLE and that reading the status clears it.

### §2.1 Entity: get_instance_handle

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity
— "get_instance_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::instance_handle`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `instance_handle()`; `test_entity_operations` checks that six
entities have six different, non-nil handles and that the participant knows them.

---

## §2.1 Programming interfaces of [DDS] 1.2: DomainParticipantFactory

### §2.1 DomainParticipantFactory: create_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "create_participant (domain_id: DomainId_t, qos_list:
QosPolicy [], a_listener: DomainParticipantListener, mask: StatusKind []):
DomainParticipant"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::TDomainParticipant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`

**Status:** `done` — The OMG headers have no DomainParticipantFactory:
`DomainParticipant(domain_id[, qos[, listener, mask]])` creates the participant
(constructor).

### §2.1 DomainParticipantFactory: delete_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "delete_participant (a_participant:
DomainParticipant): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — The OMG headers have no factory: `close()` or the destruction of
the last reference deletes the participant
(`test_entities_and_statuses_without_other_coverage`: a participant without `retain()`
goes with its last reference).

### §2.1 DomainParticipantFactory: get_instance

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "(static) get_instance: DomainParticipantFactory"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have no DomainParticipantFactory and
no `get_instance`; by §2 the headers prevail over the PDF. The C API has
`zerodds_dpf_get_instance`, which the C++ classes call internally.

### §2.1 DomainParticipantFactory: lookup_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "lookup_participant (domain_id: DomainId_t):
DomainParticipant"

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp::dds::domain::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `dds::domain::find(domain_id)` returns the domain's participant or
a null participant; a retained participant is found even when the application holds no
reference.

### §2.1 DomainParticipantFactory: set_default_participant_qos and get_default_participant_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "set_default_participant_qos (qos_list: QosPolicy
[]): ReturnCode_t; get_default_participant_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_participant_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — The static function `DomainParticipant::default_participant_qos()`
reads and `default_participant_qos(qos)` sets the factory's default
(`test_entities_and_statuses_without_other_coverage` sets it and restores it).

### §2.1 DomainParticipantFactory: get_qos and set_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "get_qos (out: qos_list: QosPolicy []): ReturnCode_t;
set_qos (qos_list: QosPolicy []): ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have no DomainParticipantFactory and
no operation for its QoS; by §2 the headers prevail over the PDF.
ZeroDDS has no class `DomainParticipantFactoryQos`.

---

## §2.1 Programming interfaces of [DDS] 1.2: DomainParticipant

### §2.1 DomainParticipant: create_publisher and delete_publisher

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 34–35 (`dds-1.2.pdf`), table
DomainParticipant — "create_publisher (qos_list: QosPolicy [], a_listener:
PublisherListener, mask: StatusKind []): Publisher; delete_publisher
(a_publisher: Publisher): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::TPublisher`,
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Publisher(dp[, qos[, listener, mask]])` creates, `close()` or
destruction deletes; `delete_contained_entities()` of the participant closes the created
publishers (`test_participant_operations_without_other_coverage`).

### §2.1 DomainParticipant: create_subscriber and delete_subscriber

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_subscriber (qos_list: QosPolicy [], a_listener:
SubscriberListener, mask: StatusKind []): Subscriber; delete_subscriber
(a_subscriber: Subscriber): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::TSubscriber`,
`crates/cpp/include/dds/sub/detail/SubscriberDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Subscriber(dp[, qos[, listener, mask]])` creates, `close()` or
destruction deletes; `delete_contained_entities()` of the participant closes the created
subscribers.

### §2.1 DomainParticipant: create_topic and delete_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_topic (topic_name: string, type_name: string,
qos_list: QosPolicy [], a_listener: TopicListener, mask: StatusKind []): Topic;
delete_topic (a_topic: Topic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TTopic.hpp::TTopic::TTopic`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Topic<T>(dp, name[, type_name][, qos[, listener, mask]])` creates,
`close()` or destruction deletes. The type name is that of
`topic_type_support<T>::type_name()` or the one given.

### §2.1 DomainParticipant: create_contentfilteredtopic and delete_contentfilteredtopic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_contentfilteredtopic (name: string, related_topic:
Topic, filter_expression: string, expression_parameters: string []):
ContentFilteredTopic; delete_contentfilteredtopic (a_contentfilteredtopic:
ContentFilteredTopic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TContentFilteredTopic.hpp`,
`crates/cpp/include/dds/topic/detail/ContentFilteredTopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `ContentFilteredTopic<T>(topic, name, Filter)` creates, `close()`
or destruction deletes; the name is its own, `filter_expression()` and
`filter_parameters()` read.

### §2.1 DomainParticipant: create_multitopic and delete_multitopic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_multitopic (name: string, type_name: string,
subscription_expression: string, expression_parameters: string []): MultiTopic;
delete_multitopic (a_multitopic: MultiTopic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/MultiTopic.hpp`,
`crates/cpp/include/dds/topic/TMultiTopic.hpp`,
`crates/cpp/include/dds/topic/detail/MultiTopicDelegate.hpp`,
`crates/zerodds-c-api/src/multitopic_ffi.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_multitopic_joins_two_generated_topics_for_a_data_reader`,
`crates/zerodds-c-api/tests/multitopic_c.rs::a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists`

**Status:** `done` — `MultiTopic<T>(dp, name, expression, params_begin, params_end)` creates the MultiTopic (`zerodds_dp_create_multitopic`): the topics of the FROM clause are the participant's topics that know their type object; an expression, topics and result type that do not fit together give an error. `close()` deletes it (`zerodds_dp_delete_multitopic`) and gives `PreconditionNotMetError` while a reader on it exists. The test joins `Order` and `Customer` into `Sale` with `WHERE amount > %0`; one sample arrives.

### §2.1 DomainParticipant: find_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35–36 (`dds-1.2.pdf`), table
DomainParticipant — "find_topic (topic_name: string, timeout: Duration_t):
Topic"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::find_topic`,
`crates/cpp/include/dds/topic/find.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `find_topic(name)` returns the `AnyTopic` of the name, a null
`AnyTopic` for an unknown name; `dds::topic::find` looks up the participant's topics by
name.

### §2.1 DomainParticipant: lookup_topicdescription

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "lookup_topicdescription (name: string): TopicDescription"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::lookup_topicdescription`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `lookup_topicdescription(name)` returns the `AnyTopic` of the name
or a null `AnyTopic`.

### §2.1 DomainParticipant: get_builtin_subscriber

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_builtin_subscriber: Subscriber"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_builtin_subscriber`,
`crates/cpp/include/dds/sub/find.hpp::dds::sub::builtin_subscriber`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `dds::sub::builtin_subscriber(dp)` returns the `BuiltinSubscriber`; `dds::sub::find<DataReader<…BuiltinTopicData>>(builtin, "DCPSParticipant" | "DCPSTopic" | "DCPSPublication" | "DCPSSubscription", it)` gives the readers of the four built-in topics (`zerodds_builtin_subscriber_lookup_datareader`); `read()` and `take()` return their samples with SampleInfo (`test_builtin_topic_readers`; C API test `builtin_reader`).

### §2.1 DomainParticipant: ignore_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_participant (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::domain::ignore(dp, handle)` and the form with an iterator
pair call `ignore_participant` of the C API (`zerodds_dp_ignore_participant`);
`test_ignore_ends_a_match` checks that the call with the handle of a discovered entity
ends the match with it.

### §2.1 DomainParticipant: ignore_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_topic (handle: InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::topic::ignore(dp, handle)` and the form with an iterator pair
call `ignore_topic` of the C API (`zerodds_dp_ignore_topic`); `test_ignore_ends_a_match`
checks that the call with the handle of a discovered entity ends the match with it.

### §2.1 DomainParticipant: ignore_publication

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_publication (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::pub::ignore(dp, handle)` and the form with an iterator pair call `ignore_publication` of the C API (`zerodds_dp_ignore_publication`) on the reader's participant with the handle of the remote publication (handle from `matched_publications`); `test_ignore_ends_a_match` checks that the list of matches is empty afterwards.

### §2.1 DomainParticipant: ignore_subscription

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_subscription (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::sub::ignore(dp, handle)` and the form with an iterator pair call `ignore_subscription` of the C API (`zerodds_dp_ignore_subscription`) on the writer's participant with the handle of the remote subscription (handle from `matched_subscriptions`); `test_ignore_ends_a_match` checks that the list of matches is empty afterwards.

### §2.1 DomainParticipant: get_domain_id

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_domain_id: DomainId_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::domain_id`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `domain_id()`.

### §2.1 DomainParticipant: delete_contained_entities

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::delete_contained_entities`,
`crates/cpp/include/dds/core/detail/ChildTracker.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `delete_contained_entities()` closes the publishers, subscribers,
topics and content-filtered topics created with the participant and still held by the
application, writers and readers first, and then calls the C API function. Before, the C
API freed the children and their delegates freed them a second time on destruction. The
operation is not in the OMG headers; it is an extension of ZeroDDS.

### §2.1 DomainParticipant: assert_liveliness

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::assert_liveliness`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `assert_liveliness()` calls `zerodds_dp_assert_liveliness`; the
test checks that the call succeeds.

### §2.1 DomainParticipant: set_default_publisher_qos and get_default_publisher_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_publisher_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_publisher_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_publisher_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_publisher_qos()` reads, `default_publisher_qos(qos)` sets
(returns the participant).

### §2.1 DomainParticipant: set_default_subscriber_qos and get_default_subscriber_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_subscriber_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_subscriber_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_subscriber_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_subscriber_qos()` reads, `default_subscriber_qos(qos)`
sets.

### §2.1 DomainParticipant: set_default_topic_qos and get_default_topic_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_topic_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_topic_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_topic_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_topic_qos()` reads, `default_topic_qos(qos)` sets.

### §2.1 DomainParticipant: get_discovered_participants

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_participants (inout: participant_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_participants`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dp->get_discovered_participants()` returns the handles of the
discovered participants; the reader's participant discovers the writer's
(`test_ignore_ends_a_match`).

### §2.1 DomainParticipant: get_discovered_participant_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36–37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_participant_data (inout: participant_data:
ParticipantBuiltinTopicData, participant_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_participant_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dp->get_discovered_participant_data(handle)` returns
`ParticipantBuiltinTopicData` (test with the handle of a discovered participant); a
handle that names no discovered participant is `PreconditionNotMetError`.

### §2.1 DomainParticipant: get_discovered_topics

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_topics (inout: topic_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topics`,
`crates/cpp/include/dds/topic/discovery.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topics_empty_initially`

**Status:** `done` — `get_discovered_topics()` returns the handles; without discovered
topics the list is empty.

### §2.1 DomainParticipant: get_discovered_topic_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_topic_data (inout: topic_data:
TopicBuiltinTopicData, topic_handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topic_data`,
`crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`

**Status:** `done` — `get_discovered_topic_data(handle)` returns
`TopicBuiltinTopicData`; the class carries every member of DCPSTopic as its policy class
(`test_topic_data_carries_every_member`, commit `e0b3af9f2c7`).

### §2.1 DomainParticipant: data of a discovered topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1.30, p. 44 (`dds-1.2.pdf`) — "This
operation retrieves information on a Topic that has been discovered on the
network."

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`

**Status:** `done` — The members of `TopicBuiltinTopicData` (name, type name,
durability, durability service, deadline, latency budget, liveliness, reliability,
transport priority, lifespan, destination order, history, resource limits, ownership,
topic data) are taken from the C API's structure; the test fills the structure and
checks every member.

### §2.1 DomainParticipant: unknown handle in get_discovered_topic_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1.30, p. 44 (`dds-1.2.pdf`); §7.5.5,
p. 24 — "The topic_handle must correspond to such a topic. Otherwise, the
operation will fail and return PRECONDITION_NOT_MET."

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topic_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`

**Status:** `done` — A handle with no discovered topic is `PreconditionNotMetError`
(commit `44aa2082361`).

### §2.1 DomainParticipant: contains_entity

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "contains_entity (a_handle: InstanceHandle_t): boolean"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::contains_entity`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `contains_entity(handle)`; true for the six entities of the test,
false for them after `delete_contained_entities()`.

### §2.1 DomainParticipant: get_current_time

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_current_time (inout: current_time: Time_t):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::current_time`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `current_time()` returns the participant's time as `Time`; the test
checks that it is wall-clock time.

### §2.1 DomainParticipantListener

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.2.3, p. 47–48 (`dds-1.2.pdf`) — "The
DomainParticipantListener interface extends all other Listener interfaces and
has no additional operation beyond the ones defined by the more general
listeners."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_domain_participant_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `DomainParticipantListener` inherits `PublisherListener`,
`SubscriberListener` and `AnyTopicListener` (multiple inheritance) and has no operation
of its own; `NoOpDomainParticipantListener` is the default. Both callbacks of the
participant table (`on_inconsistent_topic`, `on_data_on_readers`) reach their virtual
method.

---

## §2.1 Programming interfaces of [DDS] 1.2: topic classes

### §2.1 TopicDescription: get_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "get_participant: DomainParticipant"

**Repo:**
`crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::domain_participant`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `domain_participant()` returns the topic's participant; the test
checks the domain id.

### §2.1 TopicDescription: get_type_name

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "readonly: type_name string; get_type_name: string"

**Repo:**
`crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::type_name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `type_name()`; the name is that of
`topic_type_support<T>::type_name()` or the one given on creation.

### §2.1 TopicDescription: get_name

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "readonly: name string; get_name: string"

**Repo:** `crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `name()` for `Topic<T>` and `ContentFilteredTopic<T>`;
`AnyTopicDescription` (`topic/AnyTopicDescription.hpp`) names the name of any
description.

### §2.1 TopicDescription: name of a ContentFilteredTopic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.1.3, p. 50 (`dds-1.2.pdf`) — "The name
used to create the TopicDescription."

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `name()` of a ContentFilteredTopic is the name it was created with,
not that of the related topic (commit `fa26fc22a7a`).

### §2.1 Topic: get_inconsistent_topic_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.2, p. 50 (`dds-1.2.pdf`), table Topic
— "get_inconsistent_topic_status (out: status: InconsistentTopicStatus):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TTopic.hpp::TTopic::inconsistent_topic_status`,
`crates/cpp/include/dds/topic/AnyTopic.hpp::AnyTopic::inconsistent_topic_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `inconsistent_topic_status()` of `Topic<T>` and `AnyTopic`; without
an inconsistency `total_count()` is zero.

### §2.1 ContentFilteredTopic: filter_expression

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51–52 (`dds-1.2.pdf`), table
ContentFilteredTopic — "readonly: filter_expression string"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::filter_expression`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `filter_expression()` returns the expression of the filter the
ContentFilteredTopic was created with.

### §2.1 ContentFilteredTopic: get_related_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51 (`dds-1.2.pdf`), table
ContentFilteredTopic — "get_related_topic: Topic"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::topic`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `topic()` returns the related topic.

### §2.1 ContentFilteredTopic: get_expression_parameters and set_expression_parameters

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51–52 (`dds-1.2.pdf`), table
ContentFilteredTopic — "get_expression_parameters (out: expression_parameters:
string []): ReturnCode_t; set_expression_parameters (expression_parameters:
string []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::filter_parameters`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `filter_parameters()` reads and `filter_parameters(begin, end)`
sets; an empty range for an expression with parameters is an error.

### §2.1 MultiTopic: subscription_expression and parameters

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.4, p. 52–53 (`dds-1.2.pdf`), table
MultiTopic — "readonly: subscription_expression string;
get_expression_parameters (out: expression_parameters: string []): ReturnCode_t;
set_expression_parameters (expression_parameters: string []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/MultiTopic.hpp`,
`crates/cpp/include/dds/topic/TMultiTopic.hpp`,
`crates/cpp/include/dds/topic/detail/MultiTopicDelegate.hpp`,
`crates/zerodds-c-api/src/multitopic_ffi.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_multitopic_joins_two_generated_topics_for_a_data_reader`,
`crates/zerodds-c-api/tests/multitopic_c.rs::a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists`

**Status:** `done` — `MultiTopic::expression()` gives the expression, `expression_parameters()` the parameters, `expression_parameters(begin, end)` replaces them (`zerodds_multitopic_set_parameters`); the test reads them, replaces them and puts them back.

### §2.1 TopicListener: on_inconsistent_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.5, p. 53 (`dds-1.2.pdf`), table
TopicListener — "on_inconsistent_topic (the_topic: Topic, status:
InconsistentTopicStatus): void"

**Repo:** `crates/cpp/include/dds/topic/TopicListener.hpp`,
`crates/cpp/include/dds/topic/AnyTopicListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `TopicListener<T>::on_inconsistent_topic(Topic<T>&, status)` and
`NoOpTopicListener<T>`; the entry of the topic's table (`zerodds_topic_get_listener`)
calls the virtual method with the topic; the participant's entry calls
`AnyTopicListener::on_inconsistent_topic` with an `AnyTopic`.

### §2.1 TypeSupport: register_type and get_type_name

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.3.6, p. 53–54 (`dds-1.2.pdf`), table
TypeSupport; §7.8, p. 28 — "register_type (participant: DomainParticipant,
type_name: string): ReturnCode_t; get_type_name: string"

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp::topic_type_support`,
`crates/cpp/include/dds/topic/TTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — The PSM has no TypeSupport class: `Topic<T>` registers the type on
creation (`topic_type_support<T>`), `type_name()` names it; `Topic<DynamicData>`
registers through `register_type` (`core/xtypes/DynamicTopic.hpp`).

---

## §2.1 Programming interfaces of [DDS] 1.2: Publisher

### §2.1 Publisher: create_datawriter and delete_datawriter

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "create_datawriter (a_topic: Topic, qos: QosPolicy [], a_listener:
DataWriterListener, mask: StatusKind []): DataWriter; delete_datawriter
(a_datawriter: DataWriter): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::DataWriter`,
`crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `DataWriter<T>(pub, topic[, qos[, listener, mask]])` creates,
`close()` or destruction deletes.

### §2.1 Publisher: lookup_datawriter

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "lookup_datawriter (topic_name: string): DataWriter"

**Repo:** `crates/cpp/include/dds/pub/find.hpp::dds::pub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::pub::find<DataWriter<T>>(pub, topic_name, …)` returns the
publisher's writers for the topic name (iterator form and form with a maximum), also as
`AnyDataWriter`; an unknown name gives no writer.

### §2.1 Publisher: suspend_publications and resume_publications

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "suspend_publications: ReturnCode_t; resume_publications:
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp::PublisherDelegate::suspend_publications`,
`crates/cpp/include/dds/pub/SuspendedPublication.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `suspend_publications()`, `resume_publications()` and the RAII
class `SuspendedPublication`; the test uses the class (begin and end without error).

### §2.1 Publisher: begin_coherent_changes and end_coherent_changes

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "begin_coherent_changes: ReturnCode_t; end_coherent_changes:
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp::PublisherDelegate::begin_coherent_changes`,
`crates/cpp/include/dds/pub/CoherentSet.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `begin_coherent_changes()`, `end_coherent_changes()` and the RAII
class `CoherentSet`; the test uses the class. This test does not check the effect at the
reader (coherent delivery).

### §2.1 Publisher: wait_for_acknowledgments

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::wait_for_acknowledgments`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_acknowledgments(timeout)` waits for the acknowledgment of
the samples of a writer with a matching reader and throws `TimeoutError` on timeout.

### §2.1 Publisher: get_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "get_participant: DomainParticipant"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::participant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `participant()` returns the publisher's participant.

### §2.1 Publisher: delete_contained_entities

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have this operation on no class (not
on the participant either); by §2 they prevail over the PDF. Closing the participant
through its `delete_contained_entities()` (an extension of ZeroDDS) closes the writers
of every publisher.

### §2.1 Publisher: set_default_datawriter_qos and get_default_datawriter_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57–58 (`dds-1.2.pdf`), table
Publisher — "set_default_datawriter_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datawriter_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::default_writer_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `default_writer_qos()` reads, `default_writer_qos(qos)` sets the
default of the publisher's writers.

### §2.1 Publisher: copy_from_topic_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 58 (`dds-1.2.pdf`), table
Publisher — "copy_from_topic_qos (inout: a_datawriter_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/qos/DataWriterQos.hpp::DataWriterQos::DataWriterQos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `DataWriterQos(topic_qos)` builds the writer QoS from the topic QoS
(constructor from another QoS kind); `test_qos_composition` checks the carry-over.

### §2.1 PublisherListener

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.3, p. 71–72 (`dds-1.2.pdf`) —
"PublisherListener: no attributes, no operations (inherits the operations of
DataWriterListener)"

**Repo:** `crates/cpp/include/dds/pub/PublisherListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — `PublisherListener` inherits `AnyDataWriterListener` and has no
operation of its own; `NoOpPublisherListener` is the default. Each of the four entries
of the publisher's table (`zerodds_pub_get_listener`) calls its virtual method with an
`AnyDataWriter` of the triggering writer.

---

## §2.1 Programming interfaces of [DDS] 1.2: DataWriter

### §2.1 DataWriter: register_instance and register_instance_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "register_instance (instance: Data): InstanceHandle_t;
register_instance_w_timestamp (instance: Data, timestamp: Time_t):
InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::register_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `register_instance(sample)` and `register_instance(sample, time)`
return the handle of the key's instance; the same instance on repeated registration.
Tested with a generated type (`generated_type_roundtrip.cpp`, through
`generated_type_via_cargo`) and with a built-in type with a key
(`test_instance_operations_with_a_time_stamp`).

### §2.1 DataWriter: unregister_instance and unregister_instance_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "unregister_instance (instance: Data, handle: InstanceHandle_t):
ReturnCode_t; unregister_instance_w_timestamp (instance: Data, handle:
InstanceHandle_t, timestamp: Time_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::unregister_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `unregister_instance(handle[, time])`.

### §2.1 DataWriter: get_key_value

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `key_value(sample, handle)` and `key_value(TopicInstance, handle)`
return the key of a registered instance; a handle that names no instance of the writer
is an exception.

### §2.1 DataWriter: lookup_instance

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::lookup_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `lookup_instance(sample)` returns the handle of the key's instance.

### §2.1 DataWriter: write and write_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "write (data: Data, handle: InstanceHandle_t): ReturnCode_t;
write_w_timestamp (data: Data, handle: InstanceHandle_t, source_timestamp:
Time_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_read_keeps_samples_take_removes_them`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(sample)`, `write(sample, time)`, `operator<<`, iterator
ranges with and without a time and `write(TopicInstance)`; the test checks that every
form delivers its sample.

### §2.1 DataWriter: write with a handle, write_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2.11–12, p. 67–68 (`dds-1.2.pdf`) —
"The special value HANDLE_NIL can be used for the parameter handle. This
indicates that the identity of the instance should be automatically deduced from
the instance_data (by means of the key)."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `write(sample, handle[, time])` writes to the named instance; a
handle that is not the sample's instance is `InvalidArgumentError`.

### §2.1 DataWriter: dispose and dispose_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t;
dispose_w_timestamp (data: Data, handle: InstanceHandle_t, timestamp: Time_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::dispose_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `dispose_instance(handle[, time])`; the reader sees the instance as
NOT_ALIVE_DISPOSED (`generated_type_roundtrip.cpp`).

### §2.1 DataWriter: dispose with a handle, dispose_w_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::dispose_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `dispose_instance(handle[, time])` takes the instance's handle.

### §2.1 DataWriter: wait_for_acknowledgments

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::wait_for_acknowledgments`,
`crates/cpp/include/dds/pub/AnyDataWriter.hpp::AnyDataWriter::wait_for_acknowledgments`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_acknowledgments(timeout)` on the DataWriter and on the
AnyDataWriter.

### §2.1 DataWriter: get_liveliness_lost_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_liveliness_lost_status (out: status: LivelinessLostStatus):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::liveliness_lost_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `liveliness_lost_status()`; a writer that lost no liveliness has
`total_count() == 0`.

### §2.1 DataWriter: get_offered_deadline_missed_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_offered_deadline_missed_status (out: status:
OfferedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::offered_deadline_missed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `offered_deadline_missed_status()` and, under the name in the OMG
headers, `offered_deadlined_missed_status()`.

### §2.1 DataWriter: get_offered_incompatible_qos_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_offered_incompatible_qos_status (out: status:
OfferedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::offered_incompatible_qos_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `offered_incompatible_qos_status()` and, under the name in the OMG
headers, `offered_incompatible_qos_staus()`.

### §2.1 DataWriter: get_publication_matched_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_publication_matched_status (out: status:
PublicationMatchedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::publication_matched_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `publication_matched_status()`; the test checks `total_count == 1`
and `current_count == 1` for a writer and a reader.

### §2.1 DataWriter: get_topic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_topic: Topic"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::topic`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `topic()` returns the writer's topic.

### §2.1 DataWriter: get_publisher

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_publisher: Publisher"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::publisher`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `publisher()` returns the writer's publisher.

### §2.1 DataWriter: assert_liveliness

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "assert_liveliness: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::assert_liveliness`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `assert_liveliness()`.

### §2.1 DataWriter: get_matched_subscription_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter; §7.1.2.4.2.23, p. 70 — "get_matched_subscription_data (inout:
subscription_data: SubscriptionBuiltinTopicData, subscription_handle:
InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscription_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — `matched_subscription_data(writer, handle)` returns the reader's
`SubscriptionBuiltinTopicData`; a handle that names no reader is `InvalidArgumentError`.

### §2.1 DataWriter: get_matched_subscriptions

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_matched_subscriptions (inout: subscription_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscriptions`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `matched_subscriptions(writer)` and the iterator form return the
handles of the matching readers.

---

## §2.1 Programming interfaces of [DDS] 1.2: DataWriterListener

### §2.1 DataWriterListener: on_liveliness_lost

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_liveliness_lost (the_writer: DataWriter, status:
LivelinessLostStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_liveliness_lost`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — The shim of the C API's table calls `on_liveliness_lost(writer,
status)` with the writer that triggers it.

### §2.1 DataWriterListener: on_offered_deadline_missed

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_offered_deadline_missed (the_writer: DataWriter,
status: OfferedDeadlineMissedStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_offered_deadline_missed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The shim of the C API's table calls
`on_offered_deadline_missed(writer, status)` with the triggering writer.

### §2.1 DataWriterListener: on_offered_incompatible_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_offered_incompatible_qos (the_writer: DataWriter,
status: OfferedIncompatibleQosStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_offered_incompatible_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The shim of the C API's table calls
`on_offered_incompatible_qos(writer, status)` with the triggering writer.

### §2.1 DataWriterListener: on_publication_matched

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_publication_matched (the_writer: DataWriter, status:
PublicationMatchedStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_publication_matched`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`

**Status:** `done` — The shim calls `on_publication_matched(writer, status)` with the
writer and its `PublicationMatchedStatus`.

### §2.1 Listener: triggering entity and status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72, §7.1.2.5.7, p. 105
(`dds-1.2.pdf`); §7.3, p. 17 — "on_publication_matched (the_writer: DataWriter,
status: PublicationMatchedStatus): void"

**Repo:** `crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp`,
`crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Every callback gets the entity that triggers it, not a stand-in
object, and its status (commit `24f7d012de2`); the table is per entity and is
deregistered on destruction.

---

## §2.1 Programming interfaces of [DDS] 1.2: Subscriber

### §2.1 Subscriber: create_datareader and delete_datareader

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80 (`dds-1.2.pdf`), table
Subscriber — "create_datareader (a_topic: TopicDescription, qos: QosPolicy [],
a_listener: DataReaderListener, mask: StatusKind []): DataReader;
delete_datareader (a_datareader: DataReader): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/DataReader.hpp::DataReader::DataReader`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `DataReader<T>(sub, topic_description[, qos[, listener, mask]])`
creates, `close()` or destruction deletes. A ContentFilteredTopic may be the
description; a filter on fields of a type without type information (`ByteSeq`) is
refused instead of passing every sample unfiltered.

### §2.1 Subscriber: lookup_datareader

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80 (`dds-1.2.pdf`), table
Subscriber — "lookup_datareader (topic_name: string): DataReader"

**Repo:** `crates/cpp/include/dds/sub/find.hpp::dds::sub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::sub::find<DataReader<T>>(sub, topic_name, …)` returns the
subscriber's readers for the topic name (also as `AnyDataReader`).

### §2.1 Subscriber: begin_access and end_access

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80–81 (`dds-1.2.pdf`), table
Subscriber — "begin_access: ReturnCode_t; end_access: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/CoherentAccess.hpp`,
`crates/cpp/include/dds/sub/detail/SubscriberDelegate.hpp::SubscriberDelegate::begin_access`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — The RAII class `CoherentAccess(sub)` calls `begin_access`; `end()`
or destruction calls `end_access`; the test uses the class.

### §2.1 Subscriber: get_datareaders

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "get_datareaders (out: readers: DataReader [], sample_states:
SampleStateKind [], view_states: ViewStateKind [], instance_states:
InstanceStateKind []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/find.hpp::dds::sub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::sub::find<…>(sub, DataState)` selects the readers by the
masks of their samples (sample, view and instance state), `find` by description and
name; commit `ea5f1ef12d1`.

### §2.1 Subscriber: notify_datareaders

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "notify_datareaders: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::notify_datareaders`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `notify_datareaders()`; the test calls the operation.

### §2.1 Subscriber: get_participant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "get_participant: DomainParticipant"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::participant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `participant()`.

### §2.1 Subscriber: delete_contained_entities

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have this operation on no class; by §2
they prevail over the PDF. `delete_contained_entities()` of the participant (an
extension of ZeroDDS) closes the readers of every subscriber.

### §2.1 Subscriber: set_default_datareader_qos and get_default_datareader_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "set_default_datareader_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datareader_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::default_datareader_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `default_datareader_qos()` reads, `default_datareader_qos(qos)`
sets.

### §2.1 Subscriber: copy_from_topic_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "copy_from_topic_qos (inout: a_datareader_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/qos/DataReaderQos.hpp::DataReaderQos::DataReaderQos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `DataReaderQos(topic_qos)` builds the reader QoS from the topic
QoS.

### §2.1 SubscriberListener: on_data_on_readers

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.6, p. 104 (`dds-1.2.pdf`), table
SubscriberListener — "on_data_on_readers (the_subscriber: Subscriber): void"

**Repo:** `crates/cpp/include/dds/sub/SubscriberListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `SubscriberListener::on_data_on_readers(Subscriber&)` and
`NoOpSubscriberListener`; each entry of the subscriber's table
(`zerodds_sub_get_listener`) calls its virtual method, `on_data_on_readers` with the
subscriber, the others with an `AnyDataReader`.

---

## §2.1 Programming interfaces of [DDS] 1.2: DataReader

### §2.1 DataReader: read

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_read_keeps_samples_take_removes_them`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `read()` and `select()….read()` leave the samples in the reader
(commit `903a16b85f0`); forms with forward and back-insert iterators, `max_samples`, the
stream `dr >> read >> samples`.

### §2.1 DataReader: take

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "take (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_read_keeps_samples_take_removes_them`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_on_empty_returns_empty`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_returns_the_loan_when_decode_throws`

**Status:** `done` — `take()` removes the samples from the reader; an empty reader gives
an empty sequence; if the decode throws, the loan is returned (commit `409a44a86e9`).

### §2.1 DataReader: state masks in read and take

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "sample_states: SampleStateKind [], view_states: ViewStateKind [],
instance_states: InstanceStateKind []"

**Repo:** `crates/cpp/include/dds/sub/status/DataState.hpp`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::default_filter_state`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `select().state(DataState)` and `default_filter_state(DataState)`
select by sample, view and instance state; the mask values are those of DDS 1.4
(`SampleState`, `ViewState`, `InstanceState`, `DataState`); Rust tests of the C API
check the evaluation (`crates/zerodds-c-api/tests/sample_info_fields.rs`).

### §2.1 DataReader: read_w_condition and take_w_condition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read_w_condition (inout: data_values: Data [], inout:
sample_infos: SampleInfo [], max_samples: long, a_condition: ReadCondition):
ReturnCode_t; take_w_condition (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/Query.hpp`,
`crates/cpp/include/dds/sub/cond/QueryCondition.hpp`,
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`,
`crates/zerodds-c-api/tests/query_condition_read.rs::read_and_take_with_a_query_condition_return_only_the_samples_the_expression_selects`

**Status:** `done` — The PSM has no `read_w_condition`: `select().state(…)` corresponds
to the ReadCondition, `select().content(query)` to the QueryCondition (commit
`8733e3630a0`). An expression on fields selects only the matching samples
(`generated_type_roundtrip.cpp`: `value > 50`; C API: `query_condition_read.rs`).

### §2.1 DataReader: read_next_sample and take_next_sample

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read_next_sample (inout: data_value: Data, inout: sample_info:
SampleInfo): ReturnCode_t; take_next_sample (inout: data_value: Data, inout:
sample_info: SampleInfo): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — The PSM has no operation `read_next_sample`: `read(iterator, 1)`
and `select().max_samples(1)` return the next sample; `dr >> max_samples(1) >> take >>
samples` takes one (`test_samples_api`).

### §2.1 DataReader: read_instance and take_instance

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 87 (`dds-1.2.pdf`), table
DataReader — "read_instance (..., a_handle: InstanceHandle_t, sample_states,
view_states, instance_states): ReturnCode_t; take_instance (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `select().instance(handle).read()` and `.take()` select the samples
of one instance (`test_instance_operations_with_a_time_stamp`; together with a filter in
`generated_type_roundtrip.cpp`).

### §2.1 DataReader: read_next_instance and take_next_instance

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 87 (`dds-1.2.pdf`), table
DataReader — "read_next_instance (..., previous_handle: InstanceHandle_t,
sample_states, view_states, instance_states): ReturnCode_t; take_next_instance
(...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `select().next_instance(handle).read()` and `.take()` select the
samples of the instance after the handle (nil handle: the first).

### §2.1 DataReader: read_next_instance_w_condition and take_next_instance_w_condition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "read_next_instance_w_condition (..., previous_handle:
InstanceHandle_t, a_condition: ReadCondition): ReturnCode_t;
take_next_instance_w_condition (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `select().next_instance(handle).content(query)` combines both
(commit `eb8bfdd601e`); on an empty reader it returns nothing.

### §2.1 DataReader: return_loan

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "return_loan (inout: data_values: Data [], inout: sample_infos:
SampleInfo []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp`,
`crates/cpp/include/dds/sub/detail/SampleDetail.hpp::LoanGuard`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_returns_the_loan_when_decode_throws`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — The loan is returned with `LoanedSamples`, also when the decode
throws (`LoanGuard`); `return_loan` is not an operation of the PSM.

### §2.1 DataReader: get_key_value

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `key_value(sample, handle)` and `key_value(handle)` (as a
`TopicInstance`) return the key of an instance of the reader.

### §2.1 DataReader: lookup_instance

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::lookup_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `lookup_instance(sample)` returns the handle of the key's instance
in the reader.

### §2.1 DataReader: create_readcondition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "create_readcondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind []):
ReadCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`,
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `ReadCondition(reader, DataState[, functor])`; `state_filter()` and
`data_reader()` return the mask and the reader (commit `b221bbd356b`).

### §2.1 DataReader: create_querycondition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "create_querycondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind [],
query_expression: string, query_parameters: string []): QueryCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/QueryCondition.hpp`,
`crates/cpp/include/dds/sub/Query.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `QueryCondition(query, DataState)`; expression and parameters are
read, replaced and extended (`expression`, `parameters`, `add_parameter`; commit
`eb8bfdd601e`).

### §2.1 DataReader: delete_readcondition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "delete_readcondition (a_condition: ReadCondition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`,
`crates/cpp/include/dds/sub/cond/detail/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — The condition is deleted with its last reference (the PSM has no
`delete_readcondition`); the test creates and destroys it.

### §2.1 DataReader: get_liveliness_changed_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_liveliness_changed_status (out: status:
LivelinessChangedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::liveliness_changed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `liveliness_changed_status()`.

### §2.1 DataReader: get_requested_deadline_missed_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_requested_deadline_missed_status (out: status:
RequestedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::requested_deadline_missed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `requested_deadline_missed_status()`.

### §2.1 DataReader: get_requested_incompatible_qos_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_requested_incompatible_qos_status (out: status:
RequestedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::requested_incompatible_qos_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `requested_incompatible_qos_status()`.

### §2.1 DataReader: get_sample_lost_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_sample_lost_status (out: status: SampleLostStatus):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::sample_lost_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `sample_lost_status()`; without a loss `total_count() == 0`.

### §2.1 DataReader: get_sample_rejected_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_sample_rejected_status (out: status: SampleRejectedStatus):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::sample_rejected_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `sample_rejected_status()`.

### §2.1 DataReader: get_subscription_matched_status

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_subscription_matched_status (out: status:
SubscriptionMatchedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::subscription_matched_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `subscription_matched_status()`.

### §2.1 DataReader: get_topicdescription

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_topicdescription: TopicDescription"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::topic_description`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `topic_description()`.

### §2.1 DataReader: get_subscriber

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_subscriber: Subscriber"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::subscriber`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `subscriber()`.

### §2.1 DataReader: delete_contained_entities

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have this operation on no class; by §2
they prevail over the PDF. A reader's conditions are objects of their own, deleted with
their last reference.

### §2.1 DataReader: wait_for_historical_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "wait_for_historical_data (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::wait_for_historical_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_historical_data(timeout)`; `TimeoutError` on timeout.

### §2.1 DataReader: get_matched_publication_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader; §7.1.2.5.3.33, p. 102 — "get_matched_publication_data (inout:
publication_data: PublicationBuiltinTopicData, publication_handle:
InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/discovery.hpp::dds::sub::matched_publication_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — `matched_publication_data(reader, handle)` and, under the name in
the OMG headers, `matched_publications_data`.

### §2.1 DataReader: get_matched_publications

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_matched_publications (inout: publication_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/discovery.hpp::dds::sub::matched_publications`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `matched_publications(reader)` and the iterator form.

---

## §2.1 Programming interfaces of [DDS] 1.2: SampleInfo

### §2.1 SampleInfo: sample_state

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "sample_state: SampleStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`,
`crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `info.state().sample_state()` returns the state; the states come
from the bits of the C API (`SampleDetail.hpp::make_sample_info`); `SampleInfo` is a
value type whose parts `test_sample_info_parts_qos_provider_kinds_and_filter_expression`
builds from values and reads.

### §2.1 SampleInfo: view_state

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "view_state: ViewStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`,
`crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `info.state().view_state()` returns the state; the states come from
the bits of the C API (`SampleDetail.hpp::make_sample_info`); `SampleInfo` is a value
type whose parts `test_sample_info_parts_qos_provider_kinds_and_filter_expression`
builds from values and reads.

### §2.1 SampleInfo: instance_state

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "instance_state: InstanceStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `info.state().instance_state()`; ALIVE in the samples test,
NOT_ALIVE_DISPOSED after `dispose_instance` (`generated_type_roundtrip.cpp`).

### §2.1 SampleInfo: disposed_generation_count

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "disposed_generation_count: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::generation_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.generation_count().disposed()`; `make_sample_info` takes the
values from the C API's structure, the counting itself is checked by
`crates/zerodds-c-api/tests/sample_info_fields.rs::disposed_generation_count_increments_on_reactivation`.

### §2.1 SampleInfo: no_writers_generation_count

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "no_writers_generation_count: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::generation_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.generation_count().no_writers()`; the value comes from the C
API's structure (Rust test: `instance_state_unregister_reports_not_alive_no_writers` in
`sample_info_fields.rs`).

### §2.1 SampleInfo: sample_rank

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "sample_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().sample()`; Rust test of the counting:
`sample_rank_counts_down_within_one_batch` in `sample_info_fields.rs`.

### §2.1 SampleInfo: generation_rank

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "generation_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().generation()`.

### §2.1 SampleInfo: absolute_generation_rank

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "absolute_generation_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().absolute_generation()`.

### §2.1 SampleInfo: source_timestamp

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "source_timestamp: Time_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::timestamp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.timestamp()`; the time stamp of a writer (`write(sample,
time)`) reaches the SampleInfo through the C API's structure (Rust test:
`crates/zerodds-c-api/tests/write_w_timestamp.rs`).

### §2.1 SampleInfo: instance_handle

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "instance_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::instance_handle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `info.instance_handle()`; two samples of the same key have the same
handle (`generated_type_roundtrip.cpp`).

### §2.1 SampleInfo: publication_handle

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "publication_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::publication_handle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.publication_handle()`; the value comes from the C API's
structure.

### §2.1 SampleInfo: valid_data

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "valid_data: boolean"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::valid`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.valid()`; true for a sample with data.

---

## §2.1 Programming interfaces of [DDS] 1.2: DataReaderListener

### §2.1 DataReaderListener: on_data_available

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_data_available (the_reader: DataReader): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_data_available`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — The entry of the C API's table calls `on_data_available(reader)`
with the triggering reader.

### §2.1 DataReaderListener: on_sample_rejected

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_sample_rejected (the_reader: DataReader, status:
SampleRejectedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_sample_rejected`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls `on_sample_rejected(reader,
status)` with the triggering reader.

### §2.1 DataReaderListener: on_liveliness_changed

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_liveliness_changed (the_reader: DataReader, status:
LivelinessChangedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_liveliness_changed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls `on_liveliness_changed(reader,
status)` with the triggering reader.

### §2.1 DataReaderListener: on_requested_deadline_missed

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_requested_deadline_missed (the_reader: DataReader,
status: RequestedDeadlineMissedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_requested_deadline_missed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls
`on_requested_deadline_missed(reader, status)` with the triggering reader.

### §2.1 DataReaderListener: on_requested_incompatible_qos

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_requested_incompatible_qos (the_reader: DataReader,
status: RequestedIncompatibleQosStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_requested_incompatible_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls
`on_requested_incompatible_qos(reader, status)` with the triggering reader.

### §2.1 DataReaderListener: on_subscription_matched

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_subscription_matched (the_reader: DataReader, status:
SubscriptionMatchedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_subscription_matched`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls
`on_subscription_matched(reader, status)` with the triggering reader.

### §2.1 DataReaderListener: on_sample_lost

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_sample_lost (the_reader: DataReader, status:
SampleLostStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_sample_lost`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — The entry of the C API's table calls `on_sample_lost(reader,
status)` with the triggering reader.

---

## §2.1 Programming interfaces of [DDS] 1.2: conditions and WaitSet

### §2.1 Condition: get_trigger_value

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.7, p. 31–32 (`dds-1.2.pdf`), table
Condition — "get_trigger_value: boolean"

**Repo:** `crates/cpp/include/dds/core/cond/TCondition.hpp::TCondition::trigger_value`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `trigger_value()` of the GuardCondition, ReadCondition and
StatusCondition; the test checks the initial value FALSE of the GuardCondition and
setting it.

### §2.1 WaitSet: creation without a factory

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`) — "WaitSet
has no factory. It is created as an object directly by the natural means in each
language binding (e.g., using “new” in C++ or Java)."

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::TWaitSet`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `WaitSet()` creates a WaitSet without a factory (default
constructor).

### §2.1 WaitSet: attach_condition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table
WaitSet — "attach_condition (a_condition: Condition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::attach_condition`,
`crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::operator+=`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `attach_condition(cond)` and `ws += cond` attach a condition.

### §2.1 WaitSet: detach_condition

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table
WaitSet — "detach_condition (a_condition: Condition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::detach_condition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `detach_condition(cond)` is true for an attached condition and
false for one that is not attached.

### §2.1 WaitSet: wait

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table
WaitSet — "wait (inout: active_conditions: Condition [], timeout: Duration_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`,
`crates/cpp/include/dds/core/cond/detail/WaitSetDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait(timeout)`, `wait()` (unbounded) and the forms with
`ConditionSeq&` block until a condition holds; without a condition that holds
`wait(timeout)` ends with `TimeoutError`; `dispatch(timeout)` calls the handler of every
condition that holds.

### §2.1 WaitSet: result of wait

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table
WaitSet — "inout: active_conditions: Condition []"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait` returns the conditions that hold (`ConditionSeq`); the test
checks the count.

### §2.1 WaitSet: get_conditions

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table
WaitSet — "get_conditions (inout: attached_conditions: Condition []):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::conditions`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `conditions()` returns the attached conditions.

### §2.1 GuardCondition: creation without a factory, initial value FALSE

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.8, p. 32 (`dds-1.2.pdf`) —
"GuardCondition has no factory. It is created as an object directly by the
natural means in each language binding (e.g., using “new” in C++ or Java). When
first created the trigger_value is set to FALSE."

**Repo:**
`crates/cpp/include/dds/core/cond/TGuardCondition.hpp::TGuardCondition::TGuardCondition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `GuardCondition()`; `trigger_value()` is false at first.

### §2.1 GuardCondition: set_trigger_value

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.8, p. 32 (`dds-1.2.pdf`), table
GuardCondition — "set_trigger_value (value: boolean): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/core/cond/TGuardCondition.hpp::TGuardCondition::trigger_value`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `trigger_value(bool)` sets the value; an attached WaitSet sees it.

### §2.1 StatusCondition: set_enabled_statuses and get_enabled_statuses

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.9, p. 32 (`dds-1.2.pdf`), table
StatusCondition — "set_enabled_statuses (mask: StatusKind []): ReturnCode_t;
get_enabled_statuses: StatusKind []"

**Repo:**
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition::enabled_statuses`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `enabled_statuses(mask)` sets and `enabled_statuses()` reads the
mask.

### §2.1 StatusCondition: get_entity

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.1.9, p. 32 (`dds-1.2.pdf`), table
StatusCondition — "get_entity: Entity"

**Repo:**
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition::entity`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `entity()` returns the condition's entity (commit `eb8bfdd601e`).

### §2.1 ReadCondition: get_datareader

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.8, p. 106 (`dds-1.2.pdf`), table
ReadCondition — "get_datareader: DataReader"

**Repo:**
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp::TReadCondition::data_reader`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `data_reader()` returns the condition's reader.

### §2.1 ReadCondition: get_sample_state_mask, get_view_state_mask, get_instance_state_mask

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.8, p. 106 (`dds-1.2.pdf`), table
ReadCondition — "get_sample_state_mask: SampleStateKind []; get_view_state_mask:
ViewStateKind []; get_instance_state_mask: InstanceStateKind []"

**Repo:**
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp::TReadCondition::state_filter`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `state_filter()` returns the `DataState` with sample, view and
instance state.

### §2.1 QueryCondition: get_query_expression

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.9, p. 107 (`dds-1.2.pdf`), table
QueryCondition — "get_query_expression: string"

**Repo:**
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::expression`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `expression()` reads and `expression(text)` replaces the
expression.

### §2.1 QueryCondition: get_query_parameters and set_query_parameters

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.2.5.9, p. 107 (`dds-1.2.pdf`), table
QueryCondition — "get_query_parameters (out: query_parameters: string []):
ReturnCode_t; set_query_parameters (query_parameters: string []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::parameters`,
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::add_parameter`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/zerodds-c-api/tests/query_condition_update.rs::a_query_condition_takes_a_new_expression_and_new_parameters`

**Status:** `done` — `parameters_length()`, `begin()`, `end()` read, `parameters(begin,
end)` replaces, `add_parameter` extends (commit `eb8bfdd601e`).

---

## §2.1 Programming interfaces of [DDS] 1.2: built-in topics

### §2.1 Built-in topics: DCPSParticipant

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 148 (`dds-1.2.pdf`) —
"DCPSParticipant: key BuiltinTopicKey_t; user_data UserDataQosPolicy"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::ParticipantBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `ParticipantBuiltinTopicData` carries `key()` and `user_data()` (also as `ParticipantTopicData`); `get_discovered_participant_data(handle)` fills it, and the reader `DataReader<ParticipantBuiltinTopicData>` of the built-in subscriber gives the samples of the topic DCPSParticipant with `read()`/`take()` (`test_builtin_topic_readers`: a participant appears, `take` removes it).

### §2.1 Built-in topics: DCPSTopic

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 148 (`dds-1.2.pdf`) — "DCPSTopic:
key, name, type_name, durability, durability_service, deadline, latency_budget,
liveliness, reliability, transport_priority, lifespan, destination_order,
history, resource_limits, ownership, topic_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `TopicBuiltinTopicData` carries every member of DCPSTopic as a policy class (commit `e0b3af9f2c7`); `get_discovered_topic_data(handle)` returns the data, and the reader `DataReader<TopicBuiltinTopicData>` gives the samples of the topic DCPSTopic (`test_builtin_topic_readers`: the topic "BuiltinReaders" appears with its name).

### §2.1 Built-in topics: DCPSPublication

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 149 (`dds-1.2.pdf`) —
"DCPSPublication: key, participant_key, topic_name, type_name, durability,
durability_service, deadline, latency_budget, liveliness, reliability, lifespan,
user_data, ownership, ownership_strength, destination_order, presentation,
partition, topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::PublicationBuiltinTopicData`,
`crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscription_data`,
`crates/cpp/include/dds/sub/discovery.hpp::dds::sub::matched_publication_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `PublicationBuiltinTopicData` carries the members of DCPSPublication (key, `participant_key()`, topic and type name, Durability, Deadline, LatencyBudget, Liveliness, Reliability, UserData, Ownership, DestinationOrder, Presentation, Partition, TopicData, GroupData, DurabilityService, Lifespan, OwnershipStrength); `matched_publication_data(reader, handle)` fills it, and the reader `DataReader<PublicationBuiltinTopicData>` gives the samples of the topic DCPSPublication (`test_builtin_topic_readers`: the publication of the other participant's writer appears with its topic name). No test reads the values of the other members (see the next item).

### §2.1 Built-in topics: remaining members of DCPSPublication

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 149 (`dds-1.2.pdf`) —
"DCPSPublication: key, participant_key, type_name, durability,
durability_service, latency_budget, liveliness, reliability, lifespan,
ownership, ownership_strength, destination_order, presentation, partition,
topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::PublicationBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp::TPublicationBuiltinTopicData`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_values_arrive`

**Status:** `done` — All members arrive with the value the writer was created with: `test_builtin_topic_values_arrive` reads the publication from the built-in reader of the other participant and checks type name, DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS, RELIABILITY, DESTINATION_ORDER, USER_DATA, OWNERSHIP, OWNERSHIP_STRENGTH, LIFESPAN, PARTITION, GROUP_DATA, TOPIC_DATA, PRESENTATION, DURABILITY_SERVICE, key and participant key. The test found that the TOPIC_DATA of the topic did not arrive; a writer and a reader now announce the TOPIC_DATA of their topic.

### §2.1 Built-in topics: DCPSSubscription

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, topic_name, type_name, durability,
deadline, latency_budget, liveliness, reliability, ownership, destination_order,
user_data, time_based_filter, presentation, partition, topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::SubscriptionBuiltinTopicData`,
`crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscription_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `SubscriptionBuiltinTopicData` carries the members of DCPSSubscription including `time_based_filter()`; `matched_subscription_data(writer, handle)` fills it, and the reader `DataReader<SubscriptionBuiltinTopicData>` gives the samples of the topic DCPSSubscription (`test_builtin_topic_readers`: the subscription of the other participant's reader appears with its topic name, seen from the writer's side).

### §2.1 Built-in topics: remaining members of DCPSSubscription

**Spec:** §2.1, p. 11; [DDS] 1.2 §7.1.5, p. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, durability, deadline, latency_budget,
liveliness, reliability, ownership, destination_order, user_data, presentation,
partition, topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::SubscriptionBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp::TSubscriptionBuiltinTopicData`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_values_arrive`

**Status:** `done` — The members arrive with the value the reader was created with: `test_builtin_topic_values_arrive` checks DURABILITY, DEADLINE, LATENCY_BUDGET, RELIABILITY, OWNERSHIP, USER_DATA, PARTITION, TIME_BASED_FILTER and TOPIC_DATA of the subscription from the built-in reader of the writer's side.

---

## §2.2 Programming Interfaces

### §2.2 Names and locations of the headers under `dds` normative

**Spec:** §2.2, p. 11 — "The file names and relative locations of all C++
headers within the “dds” directory are normative. Those headers within “detail”
subdirectories are excepted; they are not normative."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Of the 196 files of the OMG outside `detail/` (153 public) all are in the same place under `crates/cpp/include/dds/`. Compared with a script over the file lists (cache `dds-psm-cxx-1.0-sources/hpp/dds`).

### §2.2 Public names in `::dds::` normative

**Spec:** §2.2, p. 12 — "All public symbol names within the ::dds:: namespace
and its contained namespaces, including those names introduced into those
namespaces by means of typedef declarations, are normative. Those names within
“detail” namespaces are excepted; they are not normative."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_reference_type_macros`

**Status:** `done` — All identifiers of the OMG's public headers (classes, typedefs, functions, macros) exist, `OMG_DDS_REF_TYPE_BASE`, `OMG_DDS_REF_TYPE_BASE_T`, `OMG_DDS_REF_TYPE_DELEGATE_T` and `DDS_PTR_CTOR_REF_TYPE` in `dds/core/refmacros.hpp` among them (test `test_reference_type_macros`). Where the OMG headers misspell a name (`offered_deadlined_missed_status`, `offered_incompatible_qos_staus`), both spellings exist.

### §2.2 Distribution of the names over the headers normative

**Spec:** §2.2, p. 12 — "The distribution of the normative symbol names among
the normative headers is itself normative, such that a source file that includes
the header in which a given name is declared will continue to compile when that
header is replaced with the corresponding header from a different DDS
implementation."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own`

**Status:** `done` — For every name of the public OMG headers (201 names, outside `detail/`) a
translation unit that includes only the same header names it in the same namespace. All
201 are reachable. `MultiTopic` had to become reachable through `TMultiTopic.hpp` and
`dds::code` through `DynamicData.hpp`; the second spelling is a typo of the OMG headers and
exists as an alias.

### §2.2 Remaining files are a reference implementation and examples

**Spec:** §2.2, p. 12 — "The remainder of the files, declarations, and
definitions contained within this specification’s C++ programming interfaces
constitute a reference implementation and a set of examples. They are not
normative."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement about the status of the files; ZeroDDS ships
the headers under `dds/`, the headers under `zerodds/` are an API of their own.

### §2.2 No vendor extensions in normative namespaces

**Spec:** §2.2, p. 12 — "Conforming implementations shall not define
implementation-specific extension programming interfaces within normative
namespaces. They may, however, specialize normative templates defined by this
specification."

**Repo:** `crates/cpp/include/dds/core/xtypes/EnumTypes.hpp`,
`crates/cpp/include/dds/core/xtypes/CollectionTypes.hpp`,
`crates/cpp/include/dds/sub/find.hpp`,
`crates/cpp/tests/vendor_names.txt`

**Tests:** `crates/cpp/tests/public_headers.rs::no_vendor_name_in_the_normative_namespaces`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`

**Status:** `done` — `tools/psm-omg-names.py` lists the namespace-level names of our public headers
outside `detail`. Each name occurs as an identifier in the OMG headers (`omg_words.txt`) or
is listed with its reason in `vendor_names.txt` (13 entries; an entry the headers no longer
need fails the test). Removed or moved in this round: `DomainParticipantFactoryQos`
(unused, deleted), `BuiltinSubscriber` (`dds::sub::builtin_subscriber(dp)` now returns a
`dds::sub::Subscriber`, and `dds::sub::find` finds the four built-in readers on it), `TTopic`,
`TMultiTopic`, `TContentFilteredTopic`, `TTopicDescription`, `TDataReader` (now in `detail`).
The entries of `vendor_names.txt`: `dds::domain::DomainId` (the examples of §7.13 write
`DomainId id = 0;`) and twelve classes in `dds::core::xtypes` (`AliasType`, `ArrayType`,
`TArrayType`, `BitField`, `BitFlag`, `BitMaskType`, `BitSetType`, `EnumerationType`,
`Enumerator`, `OptionalAnnotation`, `TOptionalAnnotation`, `StructForwardDeclaration`); the
basis is §7.11 ("all classes necessary for describing dynamic types") with the TypeKinds of
XTypes 1.3. That is an interpretation of §7.11; the OMG headers have no class for these
kinds. Not covered: names that a macro declares.

---

## §3 Normative References

### §3 [C99] ISO/IEC 9899:1999

**Spec:** §3, p. 12 — "[C99] C Programming Language (ISO/IEC 9899:1999)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference; by §7.4.2 for the integer types of
`stdint.h`. The headers include `<stdint.h>` where they name the types globally (commit
`def4ff43f03`); see the items on §7.4.2.

### §3 [C++] ISO/IEC 14882:2003

**Spec:** §3, p. 12 — "[C++] C++ Programming Language (ISO/IEC 14882:2003)"

**Repo:** `crates/cpp/include/zerodds/psm/cxx.hpp`, `crates/cpp/include/dds/`

**Tests:**
`crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own_as_cxx03`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_as_cxx03`

**Status:** `done` — All 144 public headers under `crates/cpp/include/dds` compile on their own
with `-std=c++03 -pedantic`. Checked with g++ 12.2 (`tools/psm-cxx03-check.py --cxx g++` on the Linux test host) and with
clang, for which the C++11 extensions are errors (test
`every_public_header_compiles_on_its_own_as_cxx03`, green on the Linux test host). The smoke program
`smoke_dds_psm.cpp` is compiled with g++ as C++03 and run (test `dds_psm_cxx_smoke_as_cxx03`,
green on the Linux test host); as C++17 it runs as before. The headers use `zerodds/psm/cxx.hpp`: with a C++11
or later compiler these are the names of the standard library (`shared_ptr`, `mutex`,
`function`, `array`), with C++03 own classes (reference counting, `mutex` over POSIX threads or
Windows, `function` up to two arguments, `array`, type traits). The C++11 additions of the
specification (§7.12: move operations, initializer lists, `enum class`, `noexcept`) are behind
`#if ZERODDS_CXX11`. The tests of threads and of move operations run with C++11 only. Not C++03:
`zerodds/dds.hpp` and `zerodds/psm/convert.hpp` (the codec
`runtime/cpp/include/zerodds/wire.hpp` and the code `idlc` generates need C++17); the headers
under `dds/` do not need them (only `zerodds::wire::Encoding`, in C++03 an own enumeration type in
`zerodds/psm/encoding.hpp`). Not run: the CMake target `zerodds_psm_standalone_headers` with
`CXX_STANDARD 98`.

### §3 [DDS] version 1.2

**Spec:** §3, p. 12 — "[DDS] Data Distribution Service for Real-Time Systems
Specification, version 1.2 (OMG document formal/2007-01-01)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference; the DCPS operations are under §2.1 by this
version. ZeroDDS implements DCPS by DDS 1.4 (`dds-dcps-1.4.md`).

### §3 [DDS-XTypes] 1.0 Beta 1

**Spec:** §3, p. 12 — "[DDS-XTypes] Extensible and Dynamic Topic Types, version
1.0 Beta 1 (OMG document ptc/2010-05-12)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to a version that is not in the cache;
ZeroDDS implements XTypes 1.3 (`dds-xtypes-1.3.md`).

### §3 [DDS-CCM] 1.0 Beta 1

**Spec:** §3, p. 12 — "[DDS-CCM] DDS for Lightweight CCM, version 1.0 Beta 1
(OMG document ptc/2009-02-02)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to a version that is not in the cache;
`QosProvider` reads the XML QoS profiles by DDS-XML 1.0 (`dds-xml-1.0.md`).

---

## §4 Terms and Definitions

### §4 Data Centric Publish-Subscribe (DCPS)

**Spec:** §4, p. 12 — "The mandatory portion of the DDS specification used to
provide the functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §4 Data Distribution Service for Real-Time Systems (DDS)

**Spec:** §4, p. 12 — "An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness and
reliability. It is independent of implementation languages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §4 Data Local Reconstruction Layer

**Spec:** §4, p. 12 — "The optional portion of the DDS specification used to
provide the functionality required for an application for direct access to data
exchanged at the DCPS layer. This later builds upon the DCPS layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §4 Platform-Independent Model (PIM)

**Spec:** §4, p. 13 — "An abstract definition of a facility, often expressed
with the aid of formal or semi-formal modeling languages such as OMG UML that
does not depend on any particular implementation technology."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §4 Platform-Specific Model (PSM)

**Spec:** §4, p. 13 — "A concrete definition of a facility, typically based on a
corresponding PIM, in which all implementation-specific dependencies have been
resolved."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

---

## §5 Symbols

### §5 Symbol `<:`

**Spec:** §5, p. 13 — "The symbol “<:” is the commonly used symbol to denote
subtyping. Given two programming language type T and Q, we can say that Q <: T
if any occurrence of T can be replaced by Q."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §5 Symbol `Foo<+T>`

**Spec:** §5, p. 13 — "When Foo is a class parameterized on the type T, we use
the notation Foo<+T> to indicate that Foo is covariant in T. This means that
given Q <: T then Foo<Q> <: Foo<T> When no annotation is provided then the class
is supposed to be invariant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §5 Symbol `Foo<-T>`

**Spec:** §5, p. 13 — "When Foo is a class parameterized on the type T, we use
the notation Foo<- T> to indicate that Foo is contra-variant in T. This means
that given Q <: T then Foo<T> <: Foo<Q> When no annotation is provided then the
class is supposed to be invariant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

### §5 Symbol `Foo<T>`

**Spec:** §5, p. 13 — "When foo is non-variant in T."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

---

## §6 Additional Information

### §6.1 Acknowledgments

**Spec:** §6.1, p. 13 — "The following companies submitted this specification: •
PrismTech Corporation, Ltd. • Real-Time Innovations, Inc. (RTI)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition or notation of the specification; no
requirement.

---

## §7.1 Overview

### §7.1 Motivation: a native C++ API instead of the IDL-derived one

**Spec:** §7.1, p. 15 — "The “ISO/IEC C++ Language DDS PSM” (DDS-PSM-Cxx) was
motivated by mainly two reasons. First the IDL-derived C++ API for DDS does not
integrate well with the C++ language [...] As a result this specification takes
a completely fresh look at how a native C++ PSM can be derived from the DDS
PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Rationale of the specification; no requirement.

### §7.1 No C++11 features required, their use enabled

**Spec:** §7.1, p. 15 — "This specification does not require C++11 features for
its implementation, yet it is designed to enable the use of C++11 features, such
as the auto keyword, range-based for loops, etc."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement on the design: the headers do not need C++11 (they compile as C++03, see §3 [C++]); with a C++11 compiler `auto`, range-based `for` loops and move semantics can be used (see §7.12).

---

## §7.2 Specification Organization

### §7.2 Namespaces after the modules of the DDS 1.2 PIM

**Spec:** §7.2, p. 15 — "The DDS-PSM-Cxx API is organized around namespaces that
match the different modules defined by the DDS v1.2 PIM (see Figure 7.1)."

**Repo:** `crates/cpp/include/dds/domain/`, `crates/cpp/include/dds/topic/`,
`crates/cpp/include/dds/pub/`, `crates/cpp/include/dds/sub/`,
`crates/cpp/include/dds/core/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::core`, `dds::domain`, `dds::topic`, `dds::pub`, `dds::sub`
with the directories and namespaces of the OMG headers.

### §7.2 Namespaces below the modules after Figure 7.1

**Spec:** §7.2, p. 15, Figure 7.1 — "Figure 7.1 - Standard Packages
Organization" — core: xtypes, policy, detail, cond, status; domain: detail, qos;
Topic: qos, detail; pub: qos, detail; sub: qos, cond, status, detail

**Repo:** `crates/cpp/include/dds/core/xtypes/`, `crates/cpp/include/dds/core/policy/`,
`crates/cpp/include/dds/core/cond/`, `crates/cpp/include/dds/core/status/`,
`crates/cpp/include/dds/domain/qos/`, `crates/cpp/include/dds/topic/qos/`,
`crates/cpp/include/dds/pub/qos/`, `crates/cpp/include/dds/sub/qos/`,
`crates/cpp/include/dds/sub/cond/`, `crates/cpp/include/dds/sub/status/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — The namespaces `core::{xtypes, policy, cond, status}`,
`domain::qos`, `topic::qos`, `pub::qos`, `sub::{qos, cond, status}` and `detail` in each
exist and the tests use them.

### §7.2 Namespace `dds::sub::status`

**Spec:** §7.2, p. 15, Figure 7.1; §7.13.2, p. 32 — "sub: qos, cond, status,
detail"; ".data(dds::sub::status::DataState::new_data())"

**Repo:** `crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::sub::status::DataState` with `SampleState`, `ViewState`,
`InstanceState` and `DataState::new_data()`, `::any()`.

### §7.2 `dds::core` holds Time, Duration, policies, reference and value types

**Spec:** §7.2, p. 15 — "The dds::core - as implied by its name - provides core
abstractions that are used throughout the API, such as the Time and Duration,
the Policies, and the definition of reference and value types."

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`,
`crates/cpp/include/dds/core/policy/CorePolicy.hpp`,
`crates/cpp/include/dds/core/Reference.hpp`, `crates/cpp/include/dds/core/Value.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core` contains `Time`, `Duration`, the policies,
`Reference<D>`, `WeakReference<T>` and `Value<D>`.

### §7.2 Type constructors with a delegate parameter

**Spec:** §7.2, p. 15 — "The specification defines type constructors, i.e.,
parameterized class, that delegate their behavior to a delegate type parameter.
The standard API is turned into an implementation by properly instantiating
these type constructors with implementation provided delegates."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp`,
`crates/cpp/include/dds/pub/TPublisher.hpp`,
`crates/cpp/include/dds/sub/TSubscriber.hpp`, `crates/cpp/include/dds/topic/TTopic.hpp`,
`crates/cpp/include/dds/sub/TDataReader.hpp`,
`crates/cpp/include/dds/core/cond/TWaitSet.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — The classes are templates over a delegate
(`TDomainParticipant<DELEGATE>`, `TPublisher<DELEGATE>`, …, commits `ee3c3473721`,
`0c023caded7`); the delegates are in `detail/`.

### §7.2 `detail` sub-packages are not compliance points

**Spec:** §7.2, p. 15 — "The “detail” sub-packages visible in Figure 7.1, are
intended to store the “link” between the standard API and the vendor
implementation. The content of the detail sub-package is provided as a guideline
and does not constitute a point of compliance."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement of the specification; ZeroDDS's delegates
are in the `detail` sub-packages.

### §7.2 Packages with few dependencies

**Spec:** §7.2, p. 15 — "The DDS-PSM-Cxx organizes DDS classes as a set of
packages that maximize the coherence and minimize the dependencies across
packages. This organization minimizes API dependencies and reduce the include
files required by publish, or subscribe, only applications speeding up
compilation times."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement on the design; every public header compiles
on its own (CMake target `zerodds_psm_standalone_headers`).

### §7.2 Type constructor `TInstanceHandle` in `dds/core/TInstanceHandle.hpp`

**Spec:** §7.2, p. 15–16 — "For instance if we take as an example the type
constructor TInstanceHandle, specified in the file dds/core/TInstanceHandle.hpp
as: namespace dds { namespace core { template <typename DELEGATE> class
TInstanceHandle ; } }"

**Repo:** `crates/cpp/include/dds/core/TInstanceHandle.hpp::TInstanceHandle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `dds::core::TInstanceHandle<DELEGATE>` in
`dds/core/TInstanceHandle.hpp`; `nil()`, `is_nil()`, `value()`.

### §7.2 Instantiation by the vendor in `detail`

**Spec:** §7.2, p. 16 — "Then its instantiation is to be defined by the
implementor of the API within the dds::core::delegate namespace as something
like: namespace dds { namespace core { namespace detail { typedef
dds::core::TInstanceHandle<foo::core::InstanceHandleDelegate> InstanceHandle; }
}"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example in the specification; ZeroDDS instantiates the
type constructors in `detail` namespaces (e.g. the `InstanceHandle` delegate in
`dds/core/InstanceHandle.hpp`).

### §7.2 `dds::core::InstanceHandle` in `dds/core/InstanceHandle.hpp`

**Spec:** §7.2, p. 16 — "This instantiation of the type constructor
TInstanceHandle is then used by the standard API in the
dds/core/InstanceHandle.hpp file to define the standard instance handle as:
namespace dds { namespace core { typedef detail::InstanceHandle InstanceHandle;
} }"

**Repo:** `crates/cpp/include/dds/core/InstanceHandle.hpp::InstanceHandle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::InstanceHandle` is the instantiation of
`TInstanceHandle` in `dds/core/InstanceHandle.hpp`.

### §7.2 No vendor changes the public API

**Spec:** §7.2, p. 16 — "Under no circumstances a vendor shall change the public
API defined by this specification."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_signatures_follow_the_normative_headers`,
`crates/cpp/tests/public_headers.rs::no_vendor_name_in_the_normative_namespaces`

**Status:** `done` — Names and distribution match the OMG headers (see §2.2); the member signatures
were compared with `tools/psm-signature-diff.py` and aligned (see §2); the additional names
are in the item "§2.2 No vendor extensions in normative namespaces". Not compared:
exception specifications, default arguments.

### §7.2 Type constructors only delegate, extensions in the delegate

**Spec:** §7.2, p. 16 — "The only action performed by type constructor is to
delegate their implementation to the DELEGATE template parameter. It is the
DELEGATE type that provides the actual implementation and that encapsulate
vendor extensions."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — The type constructors call the delegate; the extensions are in the
delegate and reachable through `operator->` (`dp->find_topic(…)`,
`dw->wait_for_matched(…)`).

### §7.2 Standard way to vendor extensions

**Spec:** §7.2, p. 16 — "The DDS-PSM-Cxx API provides a standard way of
accessing vendor specific extensions."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::operator->`,
`crates/cpp/include/dds/core/Value.hpp::Value::operator->`,
`crates/cpp/include/dds/core/Reference.hpp::Reference::delegate`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `operator->` and `delegate()` give the delegate (§7.6, p. 25);
`dp->delete_contained_entities()`, `dw->wait_for_matched(…)`.

### §7.2 Including the entire API through `dds/dds.hpp`

**Spec:** §7.2, p. 16 — "1. The entire DDS API can be included at once: •
#include <dds/dds.hpp>"

**Repo:** `crates/cpp/include/dds/dds.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `#include <dds/dds.hpp>` includes the whole API; the test includes
only this header (and the headers of the additional features).

### §7.2 Including a module through `dds/module/ddsmodule.hpp`

**Spec:** §7.2, p. 16 — "2. Individual DDS modules can be included. These
headers have the form dds/module/ddsmodule.hpp. For example: • #include
<dds/pub/ddspub.hpp>"

**Repo:** `crates/cpp/include/dds/core/ddscore.hpp`,
`crates/cpp/include/dds/domain/ddsdomain.hpp`,
`crates/cpp/include/dds/topic/ddstopic.hpp`, `crates/cpp/include/dds/pub/ddspub.hpp`,
`crates/cpp/include/dds/sub/ddssub.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — The module headers `dds/core/ddscore.hpp`,
`dds/domain/ddsdomain.hpp`, `dds/topic/ddstopic.hpp`, `dds/pub/ddspub.hpp`,
`dds/sub/ddssub.hpp` exist and compile on their own.

### §7.2 Including a class through `dds/module/ClassName.hpp`

**Spec:** §7.2, p. 16 — "3. Individual types can be included. These headers have
the form dds/module/ClassName.hpp. For example: • #include
<dds/pub/DataWriter.hpp>"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp`,
`crates/cpp/include/dds/sub/DataReader.hpp`,
`crates/cpp/include/dds/domain/DomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Every class has its header (`dds/pub/DataWriter.hpp`, …), and every
public header compiles on its own (CMake target `zerodds_psm_standalone_headers`).

---

## §7.3 Concurrency, Reentrancy and Exception Safety

### §7.3 DataReader and DataWriter operations reentrant

**Spec:** §7.3, p. 16 — "All DataReader and DataWriter operations shall be
reentrant."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_operations_are_called_from_several_threads`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_under_thread_sanitizer`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::the_threaded_smoke_tests_pass_without_the_sanitizer`

**Status:** `done` — Four writer threads on one DataWriter, a thread that reads status and QoS, and a reader that takes: all 200 samples arrive, no operation throws. The test also runs with `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 reports, all in the module `libzerodds` (Rust, not instrumented by the sanitizer, its locks and channels are invisible to it), none in the C++ code; the check fails as soon as a report lies outside the library. The reports inside the library are neither confirmed nor refuted.

### §7.3 Loan-based read and take exception safe

**Spec:** §7.3, p. 16 — "Loand-based read/take operation shall be exception
safe."

**Repo:** `crates/cpp/include/dds/sub/detail/SampleDetail.hpp::LoanGuard`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_returns_the_loan_when_decode_throws`

**Status:** `done` — `LoanGuard` returns the loan even when the decode throws (commit
`409a44a86e9`); the reader stays usable.

### §7.3 Constructors and copy assignment of value types exception safe

**Spec:** §7.3, p. 17 — "Constructors and copy-assignment operators of normative
classes that inherit from Value<D> and the Value<D> template itself shall
preferably be exception safe. Deviation from this norm should be carefully noted
on vendor documentation."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`

**Tests:** —

**Status:** `n/a (informative)` — A recommendation ("preferably"). `Value<D>` copies the
delegate with its copy assignment; the policy classes hold `std::vector`, `std::string`
and numbers. No test makes a copy fail, and the documentation of `crates/cpp` names no
deviation.

### §7.3 Topic, Publisher, Subscriber and DomainParticipant operations reentrant

**Spec:** §7.3, p. 17 — "All Topic (and other TopicDescription extension
interfaces), Publisher, Subscriber, and DomainParticipant operations shall be
reentrant with the exception that close may not be called on a given object
concurrently with any other call of any method on that object or on any
contained object."

**Repo:** `crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`,
`crates/cpp/include/dds/core/detail/ChildTracker.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_operations_are_called_from_several_threads`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_from_several_threads`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_under_thread_sanitizer`

**Status:** `done` — The test calls `qos()` of publisher, subscriber and topic, `current_time()` and `contains_entity()` of the participant from one thread while writers write; `test_entities_are_created_from_several_threads` creates ten topics, publishers, subscribers, writers and readers each from four threads on one participant and closes them. The directories of the delegates (`ParticipantRegistry`, `EntityTracker`, `ChildTracker`, `RetainedEntities`) are guarded by mutexes. Calling `close()` concurrently is excluded by the spec. The test also runs with `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 reports, all in the module `libzerodds` (Rust, not instrumented by the sanitizer, its locks and channels are invisible to it), none in the C++ code; the check fails as soon as a report lies outside the library. The reports inside the library are neither confirmed nor refuted.

### §7.3 DomainParticipantFactory operations reentrant

**Spec:** §7.3, p. 17 — "All DomainParticipantFactory operations shall be
reentrant with the exception that DomainParticipantFactory.close may not be
called on a given object concurrently with any other call of any method on that
object or on any contained object."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have no DomainParticipantFactory (see
"DomainParticipantFactory: get_instance"); creating a participant is
`DomainParticipant(domain_id, …)`.

### §7.3 WaitSet and Condition operations reentrant

**Spec:** §7.3, p. 17 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception that
their close() operations may not be invoked concurrently with any other method
on the same object."

**Repo:** `crates/cpp/include/dds/core/cond/detail/WaitSetDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_conditions_are_used_from_several_threads`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_under_thread_sanitizer`

**Status:** `done` — `test_conditions_are_used_from_several_threads`: four threads set and read GuardConditions, one thread waits in a WaitSet, one attaches and detaches a condition and reads `conditions()`; 25 rounds, no operation throws. The test found a defect: the list of attached conditions in `WaitSetDelegate` and the handler in `GuardConditionDelegate` had no lock, and the program crashed under concurrent attach, detach and wait (SIGSEGV); both are guarded by a mutex now. The test also runs with `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 reports, all in the module `libzerodds` (Rust, not instrumented by the sanitizer, its locks and channels are invisible to it), none in the C++ code; the check fails as soon as a report lies outside the library. The reports inside the library are neither confirmed nor refuted.

### §7.3 Within a listener call only the triggering entity

**Spec:** §7.3, p. 17 — "Code within a DDS listener callback may not safely call
any method on any DDS Entity but the one on which the status change occurred."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — A restriction on the application's code in the
callback; it requires nothing of the implementation.

### §7.3 Methods of value types may be non-reentrant

**Spec:** §7.3, p. 17 — "Any method of any value type may be non-reentrant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission; no requirement.

### §7.3 Stronger guarantees allowed

**Spec:** §7.3, p. 17 — "A Service implementation may choose to provide
unspecified stronger guarantees than the rules above."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission; no requirement.

---

## §7.4 General Rules for Mapping the DDS PIM to the DDS-PSM-Cxx

### §7.4 General rules for a subset of the classes

**Spec:** §7.4, p. 17 — "This specification defines some general rules to map
DDS PIM classes to DDS-PSM-Cxx classes. These rules are applicable to a subset
of classes, luckily the most numerous, while special mapping is required for
some of the DDS entities as described below."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction to the rules of §7.4.1 to §7.4.6; each
rule has an item of its own.

### §7.4.1 Every PIM class becomes a C++ class

**Spec:** §7.4.1, p. 17 — "As a general rule all classes included in the DDS PIM
have to be mapped into a C++ class. The specific nature of this class depends on
whether the DDS PIM element has reference or value semantics."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`,
`crates/cpp/include/dds/core/Value.hpp`, `crates/cpp/include/dds/core/TEntity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Entities, conditions and the `Any*` classes are reference types
(`Reference<DELEGATE>`, commits `ee3c3473721`, `0c023caded7`); statuses, QoS, policies,
times and samples are value types (`Value<D>`). All are classes.

### §7.4.1 No PIM class becomes a `struct`

**Spec:** §7.4.1, p. 17 — "Note – An implication of this mapping is that no DDS
PIM class ever maps to a C++ struct."

**Repo:** `crates/cpp/include/dds/core/`

**Tests:** —

**Status:** `n/a (informative)` — No class of the PIM is a `struct`; `struct` appears in
the headers only for trait classes (`smart_ptr_traits`, `is_topic_type`, …), the `_def`
structures of `safe_enum`, helpers in `detail` and the extensions `Enumerator` and
`BitField`.

---

## §7.4.2 Mapping Primitive and Container Types

### §7.4.2 Table 7.1: Boolean → `bool`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Boolean | bool"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `bool`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Char8 → `char`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Char8 | char"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `char`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Char32 → `wchar_t`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Char32 | wchar_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `wchar_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Byte → `uint8_t`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Byte | uint8_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `uint8_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int16 → `int16_t`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Int16 | int16_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `int16_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt16 → `uint16_t`

**Spec:** §7.4.2, p. 17, Table 7.1 — "UInt16 | uint16_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `uint16_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int32 → `int32_t`

**Spec:** §7.4.2, p. 17, Table 7.1 — "Int32 | int32_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `int32_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt32 → `uint32_t`

**Spec:** §7.4.2, p. 18, Table 7.1 — "UInt32 | uint32_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `uint32_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int64 → `int64_t`

**Spec:** §7.4.2, p. 18, Table 7.1 — "Int64 | int64_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `int64_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt64 → `uint64_t`

**Spec:** §7.4.2, p. 18, Table 7.1 — "UInt64 | uint64_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `uint64_t`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float64 → `double`

**Spec:** §7.4.2, p. 18, Table 7.1 — "Float64 | double"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `double`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float128 → `long double`

**Spec:** §7.4.2, p. 18, Table 7.1 — "Float128 | long double"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `long double`. The test `check_table_7_1` checks
with `static_assert` that the accessor of a generated type's member has this type. The
code generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float32 → `float`

**Spec:** §7.4.2, p. 18, Table 7.1 — "Float32 | float"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The table row is `float`. The test `check_table_7_1` checks with
`static_assert` that the accessor of a generated type's member has this type. The code
generator produces these types by default (`--cpp-mapping psm`, commit `56ff5b5f25f`); the option `--cpp-mapping native` keeps the codec's types (`map` as `std::vector<std::pair<K, V>>`, `long
double` as `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: string<Char8> → `std::string`

**Spec:** §7.4.2, p. 18, Table 7.1 — "string<Char8> | std::string"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::string` (`static_assert` in `check_table_7_1`; `std::string`
is also the type of names in the headers).

### §7.4.2 Table 7.1: string<Char32> → `std::wstring`

**Spec:** §7.4.2, p. 18, Table 7.1 — "string<Char32> | std::wstring"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::wstring` by default (`--cpp-mapping psm`; `static_assert`); the
value runs through the codec as UTF-8 (`back.wlabel() == r.wlabel()`).

### §7.4.2 Table 7.1: sequence<T> → `std::vector<T>`


**Spec:** §7.4.2, p. 18, Table 7.1 — "sequence<T> | std::vector<T>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`

**Status:** `done` — `std::vector<T>` in both mappings (`static_assert`; Rust test of
the code generator).

### §7.4.2 Table 7.1: map<K, V> → `std::map<K, V>`

**Spec:** §7.4.2, p. 18, Table 7.1 — "map<K, V> | std::map<K, V>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::map<K, V>` by default (`--cpp-mapping psm`; `static_assert`, round
trip with two entries). The option `--cpp-mapping native` stays with `std::vector<std::pair<K, V>>`
because the order of the entries is an axis of the codec
(`a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`).

### §7.4.2 Table 7.1: T[N] → `dds::core::array<T, N>`

**Spec:** §7.4.2, p. 18, Table 7.1 — "T[N] | dds::core::array<T, N>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/array.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`test_layout_headers_and_find`,
`test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::array<T, N>` by default (`--cpp-mapping psm`; `static_assert`,
multi-dimensional `cells()[1][2]`); the option `--cpp-mapping native` uses `std::array`.
`dds::core::array` has the operations of `std::array` (`fill`, `swap`, iterators,
`get<I>`, `tuple_size`, comparisons).

### §7.4.2 Integer types per `stdint.h` of [C99]

**Spec:** §7.4.2, p. 18 — "The above fixed-size integer types shall conform to
the types of the same names as defined by [C99] in the header stdint.h."

**Repo:** `crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`test_layout_headers_and_find`

**Status:** `done` — The headers include `<stdint.h>` where they name the types globally
(commit `def4ff43f03`); the `static_assert` in `check_table_7_1` checks the global
names.

### §7.4.2 Platforms without [C99] provide their own types

**Spec:** §7.4.2, p. 18 — "The presence of these types shall not be construed to
require that DDS implementations only support [C99]-compliant platforms.
Implementations for non-[C99]-compliant platforms shall provide their own
conformant integer type definitions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission for platforms without [C99]; ZeroDDS
requires `<stdint.h>`.

### §7.4.2 No further definitions of `stdint.h` implied

**Spec:** §7.4.2, p. 18 — "It shall not be construed to imply the existence of
any other definitions that would be found in the header stdint.h on a
[C99]-compliant platform or even the existence of that header itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The headers use from `<stdint.h>` only the fixed-size
integer types.

### §7.4.2 Types in the global namespace

**Spec:** §7.4.2, p. 18 — "Note that these types are defined in the global
namespace, not in the std namespace."

**Repo:** `crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`

**Status:** `done` — The types `int32_t` etc. are in the global namespace
(`static_assert(std::is_same<::int32_t, int32_t>…)`).

### §7.4.2 Bounded and unbounded sequences the same type

**Spec:** §7.4.2, p. 18 — "In addition, it is worth noticing that bounded and
unbounded sequence types map to the same C++ types."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — A `sequence<long, 4>` and a `sequence<long>` are both
`std::vector<int32_t>` (`static_assert`).

### §7.4.2 `dds::core::array` per `std::array` without move

**Spec:** §7.4.2, p. 18 — "The DDS Array type is mapped to the dds::core::array
type which is specified to conform with the std::array type specified as part of
C++11, exception made for the move operators."

**Repo:** `crates/cpp/include/dds/core/array.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::array` has the types, iterators, `fill`, `swap`, `at`,
`front`, `back`, `data`, the comparisons and the tuple interface of `std::array`; it is
not an alias of `std::array` (see §7.12).

---

## §7.4.3 Mapping Enumerations

### §7.4.3 Enumerations as a safe class, under C++11 `enum class`

**Spec:** §7.4.3, p. 18–19 — "Native enumerations in C++ are not safe. This
specification maps DDS enumerations to a safe enumeration class defined as
follows: [...] As such, for C++11 compilers, implementers may choose to map
enumeration to C++11 enumeration classes."

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp`,
`crates/cpp/include/dds/core/policy/PolicyKind.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — The policy kinds are `safe_enum<…_def>`; the generated enumerations
are `enum class` (permitted for C++11,
`an_enumeration_takes_an_underlying_type_from_its_bit_bound`).

### §7.4.3 `safe_enum`: template and base class

**Spec:** §7.4.3, p. 18 — "namespace dds { namespace core { template<typename
def, typename inner = typename def::type> class safe_enum : public def { typedef
typename def::type type; inner val; [...]"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `safe_enum<def, inner>` inherits from `def`, so the enumerators are
reachable as `Color::GREEN`; without conversion from or to `int`.

### §7.4.3 `safe_enum`: constructor from `type`

**Spec:** §7.4.3, p. 18 — "safe_enum(type v) : val(v) {}"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::safe_enum`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `safe_enum(type v)`.

### §7.4.3 `safe_enum`: `underlying()`

**Spec:** §7.4.3, p. 18 — "inner underlying() const { return val; }"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::underlying`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `underlying()`.

### §7.4.3 `safe_enum`: `operator==` and `operator!=`

**Spec:** §7.4.3, p. 18 — "bool operator == (const safe_enum & s) const; bool
operator != (const safe_enum & s) const;"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::operator==`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `operator==` and `operator!=`.

### §7.4.3 `safe_enum`: `operator<`, `<=`, `>`, `>=`

**Spec:** §7.4.3, p. 19 — "bool operator < (const safe_enum & s) const; bool
operator <= (const safe_enum & s) const; bool operator > (const safe_enum & s)
const; bool operator >= (const safe_enum & s) const;"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::operator<`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `operator<`, `<=`, `>`, `>=`.

### §7.4.3 Example `Color` with `Color_def`

**Spec:** §7.4.3, p. 19 — "Below we provide an example of how implementations of
this specification have to the safe_enum class to map DDS enumeration. [...]
enum Color_def__ { GREEN, WHITE, RED }; typedef dds::core::safe_enum<Color_def>
Color;"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example of the specification;
`test_value_and_reference_classes_without_other_coverage` uses it.

---

## §7.4.4 Mapping Unions

### §7.4.4 Unions per IDL2C++11 §6.13.2

**Spec:** §7.4.4, p. 19 — "DDS unions mapping is the same as the one defined by
the IDL2C++11 specification as defined in 6.13.2 of the document ptc/2012-04-03.
This choice is compatible with the use of C++03 and aligns the mapping of DDS
types to that of IDL."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`, `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/cpp/tests/generated_psm_roundtrip.cpp::check_union_mapping`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** `done` — By default (`--cpp-mapping psm`) a union is a class with `_d()` and an accessor and a
modifier per branch (`c.text("hello")` sets the branch and the discriminator,
`holder.pick().text()`); a union with a `default` branch, with one member under several
labels and with an implicit discriminator is round-tripped through the codec
(`check_union_mapping`: `static_assert`s and comparisons; the conversion reads the selected
branch only). The comparison is with IDL4 to C++ 1.0 §7.2.4.3.2, the successor of IDL2C++11
§6.13.2; ptc/2012-04-03 itself is not in the cache and was not read. The option `--cpp-mapping native`
produces a `struct` with public fields.

---

## §7.4.5 Mapping Parameters Passing and Parameters Return Rules

### §7.4.5 Native type, IN → `T`

**Spec:** §7.4.5, p. 19 — "PIM Native Type Parameter | DDS-PSM-Cxx Native Type
Parameter: IN T | T"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`

**Status:** `done` — Simple parameters pass by value (`uint32_t domain_id`,
`History::KeepLast(uint32_t)`, `int32_t` setters).

### §7.4.5 Native type, OUT → `T&`

**Spec:** §7.4.5, p. 19 — "OUT T | T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — A simple value returned through `T&` is the accessor `int32_t&
depth()` of the policies; the operations of the headers have no OUT parameter of a
simple type (results are return values).

### §7.4.5 Native type, INOUT → `T&`

**Spec:** §7.4.5, p. 19 — "INOUT T | T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — As for OUT: the `T&` accessors of the policies; no operation with
an INOUT parameter of a simple type.

### §7.4.5 Native type, return → `T`

**Spec:** §7.4.5, p. 20 — "PIM Native Return Type | DDS-PSM-Cxx Native Return
Type: T | T"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::status_changes`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::domain_id`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — Simple return values go by value (`domain_id()`, `trigger_value()`,
`contains_entity()`).

### §7.4.5 Non-primitive type, IN → `const T&`

**Spec:** §7.4.5, p. 20 — "PIM Type Parameter | DDS-PSM-Cxx Type Parameter: IN T
| const T&"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::TTopic`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`

**Status:** `done` — Samples, names, handles, QoS and durations pass as `const T&`; the
constructors and setters of the policies take `const Duration&`, `const ByteSeq&`,
`const StringSeq&`.

### §7.4.5 Policies take constructed types as `const T&`

**Spec:** §7.4.5, p. 20 — "IN T | const T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The policies take constructed types as `const T&`
(`Reliability(kind, const Duration&)`, `Deadline(const Duration&)`, `UserData(const
ByteSeq&)`, `Partition(const StringSeq&)`). The row of the specification names `const
T&`, not the value.

### §7.4.5 Non-primitive type, OUT → `T&`

**Spec:** §7.4.5, p. 20 — "OUT T | T&"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — OUT parameters of a constructed type are `T&` (`wait(ConditionSeq&
triggered, timeout)`, `read(iterator, max)` into a container).

### §7.4.5 Non-primitive type, INOUT → `T&`

**Spec:** §7.4.5, p. 20 — "INOUT T | T&"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::key_value`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — INOUT parameters are `T&` (`key_value(T& sample, handle)`).

### §7.4.5 Non-primitive type, return → `T` or `const T&`

**Spec:** §7.4.5, p. 20 — "PIM Native Return Type | DDS-PSM-Cxx Native Return
Type: T | One of the following, depending on wether the return parameter is an
attribute or not. • T • const T&"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::qos`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::qos`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — Constructed return values are `T` or `const T&` (`qos()` as `const
Qos&`, `default_writer_qos()` by value, statuses as `const Status`).

---

## §7.4.6 Mapping Attributes

### §7.4.6 Attribute as state, getter and setter, constructor argument

**Spec:** §7.4.6, p. 20 — "Attributes defined by DDS PIM classes have to be
mapped into: • Implementation-defined state, • Getter and setter methods named
after the attribute, and • A constructor argument that allows initializing the
attribute."

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/sub/SampleInfo.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — Policies, statuses, `SampleInfo`, `Time`, `Duration` and the
built-in topic data keep their state private, have getters and setters named after the
attribute and constructors with the attributes.

### §7.4.6 Accessors instead of public fields

**Spec:** §7.4.6, p. 20 — "Getter/Setter methods shall be declared as described
in the following table."

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — The status classes, `SampleInfo`, the QoS sets (`policy<P>()`) and
the built-in topic data have no public fields but accessors. Only `Enumerator` and
`BitField` (extensions) have public fields.

### §7.4.6 Native type: `NT attribute();` and `void attribute(NT attrib);`

**Spec:** §7.4.6, p. 20 — "NT attribute; Where NT is a native. | NT attribute();
void attribute(NT attrib);"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/Time.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `NT attribute() const`, `NT& attribute()` and the setter
`attribute(NT)`; the policies return themselves from the setter (chainable), otherwise
`void`.

### §7.4.6 Constructed type: `CT&`, `const CT&`, `void attribute(const CT&)`

**Spec:** §7.4.6, p. 20 — "CT attribute; Where CT is a constructed type (e.g., a
struct) | CT& attribute(); const CT& attribute() const; void attribute(const CT&
attrib);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`

**Status:** `done` — `CT& attribute()`, `const CT& attribute() const` and the setter
with `const CT&` (policies: `Reliability::max_blocking_time(const Duration&)`,
`UserData::value(const ByteSeq&)`).

### §7.4.6 Sequence type: `ST&`, `const ST&`, `void attribute(const ST&)`

**Spec:** §7.4.6, p. 20 — "ST attribute; Where ST is a sequence type (e.g., a
string, sequence, map, arrays, etc.) | ST& attribute(); const ST& attribute()
const; void attribute(const ST& attrib);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — Sequences (`Partition::name`, `UserData::value`,
`TopicData::value`) have `ST&`, `const ST&` and the setter with `const ST&`.

---

## §7.5 Core Package

### §7.5 Content of the core package

**Spec:** §7.5, p. 21 — "The core package of the ISO/IEC C++ PSM for DDS
(DDS-PSM-Cxx) defines the classes at the foundation of the API object model as
well as all the DDS types used by all other modules. This sub clause describes
the most important classes of the package."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction to §7.5; each rule has an item of its
own.

---

## §7.5.1 Object Model

### §7.5.1 Two kinds of types: reference and value

**Spec:** §7.5.1, p. 21 — "The ISO/IEC C++ PSM for DDS (DDS-PSM-Cxx) is based on
an object model that is structured in two different kinds of object types:
reference-types and value-types."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`,
`crates/cpp/include/dds/core/Value.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Reference types derive from `Reference<DELEGATE>`, value types from
`Value<D>`.

---

## §7.5.1.1 Reference Types

### §7.5.1.1 Shallow assignment of a reference

**Spec:** §7.5.1.1, p. 21 — "All objects that have a reference-type have an
associated shallow (polymorphic) assignment operator that simply changes the
value of the reference."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::operator=`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Copy and assignment of a reference share the delegate
(`shared_ptr`); `test_reference_types` checks that a copied reference is the same object
and that assigning `null` releases only that reference.

### §7.5.1.1 References are safe: null or valid

**Spec:** §7.5.1.1, p. 21 — "Furthermore reference-types are safe, meaning that
under no circumstances can a reference point to an invalid object. At any single
point in time a reference can either refer to the null object or to a valid
object."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::require_delegate`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — A reference is null or valid; a call through a null reference
(`delegate()`, `operator->`, every operation of the classes) is `NullReferenceError`,
never an access through a null pointer.

### §7.5.1.1 Semantics by `dds::core::Reference`

**Spec:** §7.5.1.1, p. 21 — "The semantics for Reference types is defined by the
DDS-PSM-Cxx class dds::core::Reference. In the context of this specification the
semantics implied by the ReferenceType is mandatory, yet the implementation
provided as part of this standard is provided to show one possible way of
implementing this semantics."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `dds::core::Reference<DELEGATE>` defines the semantics; all
reference types of Table 7.2 derive from it (`static_assert` in `test_reference_types`).

### §7.5.1.1 Reference types as templates over the delegate

**Spec:** §7.5.1.1, p. 21 — "All DDS-PSM-Cxx reference-types store references to
a delegate. [...] all DDS-PSM-Cxx reference-types are template classes whose
parameter is the DELEGATE. Each vendor will plug-in his implementation simply by
providing a file that instantiates the DDS-PSM-Cxx API with its own delegates."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp`,
`crates/cpp/include/dds/pub/TPublisher.hpp`,
`crates/cpp/include/dds/sub/TSubscriber.hpp`, `crates/cpp/include/dds/topic/TTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — The reference types are templates over the delegate
(`TDomainParticipant<DELEGATE>` etc., commits `ee3c3473721`, `0c023caded7`); `Any*` hold
a holder.

### §7.5.1.1 Weak references

**Spec:** §7.5.1.1, p. 21 — "Finally, the PSM also provides weak references."

**Repo:** `crates/cpp/include/dds/core/WeakReference.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `WeakReference<T>` with `lock()` and `expired()`; `lock()` builds
`DomainParticipant` and `DataWriter<T>` from the delegate reference (constructors
`DomainParticipant(const DELEGATE_REF_T&)`, `DataWriter(const DELEGATE_REF_T&)`).

### §7.5.1.1 Table 7.2: `dds::core` Entity

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • Entity"

**Repo:** `crates/cpp/include/dds/core/Entity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::Entity` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` Condition

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • Condition"

**Repo:** `crates/cpp/include/dds/core/cond/Condition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::Condition` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` GuardCondition

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • GuardCondition"

**Repo:** `crates/cpp/include/dds/core/cond/GuardCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::GuardCondition` is a reference type: `static_assert`
in `test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` ReadCondition

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • ReadCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::cond::ReadCondition` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` QueryCondition

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • QueryCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::cond::QueryCondition` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` Waitset

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "dds | core | • Waitset"

**Repo:** `crates/cpp/include/dds/core/cond/WaitSet.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::WaitSet` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::domain` DomainParticipant

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "pomain | • DomainParticipant"

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `domain::DomainParticipant` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` AnyDataWriter

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "pub | • AnyDataWriter"

**Repo:** `crates/cpp/include/dds/pub/AnyDataWriter.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::AnyDataWriter` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` Publisher

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "pub | • Publisher"

**Repo:** `crates/cpp/include/dds/pub/Publisher.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::Publisher` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` DataWriter

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "pub | • DataWriter"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::DataWriter` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` AnyDataReader

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "sub | • AnyDataReader"

**Repo:** `crates/cpp/include/dds/sub/AnyDataReader.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::AnyDataReader` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` Subscriber

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "sub | • Subscriber"

**Repo:** `crates/cpp/include/dds/sub/Subscriber.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::Subscriber` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` DataReader

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "sub | • DataReader"

**Repo:** `crates/cpp/include/dds/sub/DataReader.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::DataReader` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` SharedSamples

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "sub | • SharedSamples"

**Repo:** `crates/cpp/include/dds/sub/SharedSamples.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::SharedSamples` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::topic` AnyTopic

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "topic | • AnyTopic"

**Repo:** `crates/cpp/include/dds/topic/AnyTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `topic::AnyTopic` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::topic` Topic

**Spec:** §7.5.1.1, p. 22, Table 7.2 — "topic | • Topic"

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `topic::Topic` is a reference type: `static_assert` in
`test_reference_types` that it derives from `Reference<DELEGATE>`; assignment,
comparison and construction with `dds::core::null`.

---

## §7.5.1.2 Resource for Reference Types

### §7.5.1.2 Creation through constructors

**Spec:** §7.5.1.2, p. 22 — "Instances of reference types are created using C++
constructors."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`,
`crates/cpp/include/dds/pub/Publisher.hpp`, `crates/cpp/include/dds/sub/Subscriber.hpp`,
`crates/cpp/include/dds/topic/Topic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — References are made through constructors (`DomainParticipant(id)`,
`Publisher(dp)`, `Topic<T>(dp, name)` …).

### §7.5.1.2 No trivial constructor for reference types

**Spec:** §7.5.1.2, p. 22 — "The trivial constructor is not defined for
reference types, the only alternative to properly constructing a reference is to
initialize it to a null reference by assigning dds::core::null."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `DomainParticipant`, `Topic`, `AnyTopic`, `ContentFilteredTopic`, `MultiTopic`, `Publisher`, `DataWriter`, `Subscriber` and `DataReader` (and their `T` bases) have no public default constructor; the null reference is made from `dds::core::null` (`static_assert` in `test_reference_types`). `WaitSet` and `GuardCondition` create a new object when default-constructed, as the OMG headers do.

### §7.5.1.2 Null reference through `dds::core::null`

**Spec:** §7.5.1.2, p. 22 — "the only alternative to properly constructing a
reference is to initialize it to a null reference by assigning dds::core::null."

**Repo:** `crates/cpp/include/dds/core/types.hpp::null`,
`crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Every reference type can be constructed from, assigned from and
compared with `dds::core::null` (`==`, `!=`).

### §7.5.1.2 `close()` halts communication and releases resources

**Spec:** §7.5.1.2, p. 22 — "These objects therefore provide a method close()
that shall halt network communication (in the case of entities) and dispose of
any appropriate operating-system resources."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::close`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `close()` deletes the writer: the reader sees it go
(`subscription_matched_status().current_count()` becomes 0), a later call is
`AlreadyClosedError`; for a retained object it ends the retention.

### §7.5.1.2 Recommendation to call `close`

**Spec:** §7.5.1.2, p. 22 — "Users of this PSM are recommended to call close on
objects of all reference types once they are finished using them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Recommendation to users; no requirement on the
implementation.

### §7.5.1.2 Directly referenced object stays in use

**Spec:** §7.5.1.2, p. 22 — "In addition, implementations may automatically
close objects that they deem to be no longer in use, subject to the following
restrictions: • Any object to which the application has a direct reference (not
including a WeakRefer- ence) is still in use."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — A writer and a reader the application holds stay usable after
participant, topic, publisher and subscriber have gone out of scope: children hold their
parents (`shared_ptr`, commit `9de21b2827b`). The test then sends a sample.

### §7.5.1.2 Entity with a listener stays in use

**Spec:** §7.5.1.2, p. 22 — "• Any entity with a non-null listener is still in
use."

**Repo:** `crates/cpp/include/dds/core/detail/Retain.hpp::retain_for_listener`,
`crates/cpp/include/dds/core/TEntity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`

**Status:** `done` — `listener(l, mask)` with a listener keeps the entity (participant,
topic, publisher, subscriber, DataWriter, DataReader) beyond its last reference;
`listener(nullptr, …)` lets it go.

### §7.5.1.2 Explicitly retained object stays in use

**Spec:** §7.5.1.2, p. 22 — "• Any object that has been explicitly retained is
still in use"

**Repo:** `crates/cpp/include/dds/core/detail/Retain.hpp`,
`crates/cpp/include/dds/core/TEntity.hpp::TEntity::retain`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `retain()` keeps the entity until `close()`; a retained participant
is found with `dds::domain::find` even without a reference of the application.

### §7.5.1.2 Creator of an entity in use stays in use

**Spec:** §7.5.1.2, p. 22 — "• The creator of any object that is still in use is
itself still in use."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — Writers and readers hold publisher, subscriber and topic, and these
hold the participant's handle (`shared_ptr` with the C API's deleter): the handle is
freed with the last child (commit `9de21b2827b`).

---

## §7.5.2 Value Types

### §7.5.2 Deep copy and assignment of value types

**Spec:** §7.5.2, p. 23 — "All objects that have a value-type have a deep-copy
assignment and copy construction semantics."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Copy and assignment of a QoS set, a policy, a status and of `Time`
and `Duration` are deep; the test changes the copy and checks the original.

### §7.5.2 Value types through `dds::core::Value`

**Spec:** §7.5.2, p. 23; §7.5.4, p. 23; §7.6.1, p. 26 — "class SampleLostStatus
: public dds::core::Value<D>"; "class THistory : public dds::core::Value<D>"

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `TSampleLostStatus<D>`, `THistory<D>` etc. derive from `Value<D>`
(commit `0c023caded7`).

### §7.5.2 Value types are mutable

**Spec:** §7.5.2, p. 23 — "It should also be pointed out that value-types are
not “pure-value-types” in the sense that they are immutable (as in functional
programming languages). The DDS-PSM-Cxx makes value-types mutable to limit the
number of copies as well limit the time-overhead necessary to change a
value-type"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Setters change policies and statuses.

### §7.5.2 All classes outside Table 7.2 are value types

**Spec:** §7.5.2, p. 23 — "The DDS-PSM-Cxx models all DDS PIM classes beyond
what is listed in Table 7.2 as value- types. In other terms, QoS, Policy,
Statuses, and Topic samples are all modeled as value-types."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/sub/SampleInfo.hpp`, `crates/cpp/include/dds/sub/Sample.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — QoS, policies, statuses, `SampleInfo`, `Sample<T>`, `Time`,
`Duration`, `InstanceHandle` and the generated topic types are copyable. For the classes
Table 7.2 does not name (ContentFilteredTopic, StatusCondition, TopicDescription) see
the following item.

### §7.5.2 ContentFilteredTopic and StatusCondition as value types

**Spec:** §7.5.2, p. 23; §7.5.1.1, p. 22, Table 7.2 — "The DDS-PSM-Cxx models
all DDS PIM classes beyond what is listed in Table 7.2 as value- types."

**Repo:** `crates/cpp/include/dds/topic/TContentFilteredTopic.hpp`,
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`

**Status:** `done` — Table 7.2 does not name `ContentFilteredTopic`, `StatusCondition`
and `TopicDescription`; the OMG headers (which prevail by §2) derive them from
`Reference` (`TContentFilteredTopic : TopicDescription : Reference`, `TStatusCondition :
TCondition`). ZeroDDS follows the headers: reference types, copyable with a shared
delegate.

---

## §7.5.3 Any Types

### §7.5.3 Few virtual methods, delegation instead of inheritance

**Spec:** §7.5.3, p. 23 — "The DDS-PSM-Cxx has been designed to take advantage
of the compile time polymorphism provided by C++ templates. As such, the whole
standard interface only has a few virtual methods, and in general does not rely
on inheritance but as opposed exploits delegation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Design goal. In ZeroDDS the methods of the listeners
and the destructor of `Condition` are virtual.

### §7.5.3 "Any" types for entities without the complete type

**Spec:** §7.5.3, p. 23 — "Since the DDS API requires at times to pass DDS
entities without exposing the complete type, while other times requires to store
in containers list of objects of different types, the DDS-PSM-Cxx provides a
selection of “Any” types. These Any types safely store references in generic
container objects without losing type information while at the same time
exposing some type-independent operations."

**Repo:** `crates/cpp/include/dds/pub/AnyDataWriter.hpp`,
`crates/cpp/include/dds/sub/AnyDataReader.hpp`,
`crates/cpp/include/dds/topic/AnyTopic.hpp`,
`crates/cpp/include/dds/topic/AnyTopicDescription.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `AnyDataWriter`, `AnyDataReader`, `AnyTopic` and
`AnyTopicDescription` hold an entity of any type; `get<T>()` returns it, another type is
`InvalidDowncastError`. `AnyDataWriter`, `AnyDataReader` and `AnyTopic` are reference
types (Table 7.2).

---

## §7.5.4 Status Classes

### §7.5.4 Status classes per §7.4 without a common base

**Spec:** §7.5.4, p. 23 — "The DDS-PSM-Cxx mapping for the status classes as
defined in the DDS v1.2 specification is obtained by applying the generic
mapping rules described in 7.4 with the following caption – inheritance from the
root status class has been ignored."

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The status classes have no common base class; each is `TXStatus<D>
: Value<D>`.

### §7.5.4 Namespace `dds::core::status`

**Spec:** §7.5.4, p. 23 — "Status classes are part of the dds::core::status
namespace."

**Repo:** `crates/cpp/include/dds/core/status/Status.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The status classes are in `dds::core::status`.

### §7.5.4 `SampleLostStatus` as a template over `Value<D>`

**Spec:** §7.5.4, p. 23 — "namespace dds { namespace core { namespace status {
template <typename D> class SampleLostStatus : public dds::core::Value<D>{"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `TSampleLostStatus<D> : Value<D>`;
`dds::core::status::SampleLostStatus` is `TSampleLostStatus<detail::SampleLostStatus>`.

### §7.5.4 `SampleLostStatus`: constructors

**Spec:** §7.5.4, p. 23 — "SampleLostStatus(); SampleLostStatus(uint32_t
total_count, uint32_t total_count_change);"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The default constructor and `SampleLostStatus(total_count,
total_count_change)`. The PDF writes `uint32_t`, the OMG headers `int32_t`; ZeroDDS
follows the headers (`int32_t`).

### §7.5.4 `SampleLostStatus`: access to `total_count`

**Spec:** §7.5.4, p. 24 — "uint32_t total_count() const; uint32_t&
total_count(); void total_count(uint32_t total_count);"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus::total_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `total_count() const`, `total_count()` as a reference and
`total_count(value)`.

### §7.5.4 All status classes in `dds/core/status/Status.hpp`

**Spec:** §7.5.4, p. 24 — "The full set of status classes is includes in the
mandatory standard headers in the file dds/core/status/Status.hpp."

**Repo:** `crates/cpp/include/dds/core/status/Status.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — All eleven status classes are in `dds/core/status/Status.hpp`.

### §7.5.4 StatusKind and StatusMask

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159 (`dds-1.2.pdf`) — "typedef
unsigned long StatusKind; typedef unsigned long StatusMask; const StatusKind
INCONSISTENT_TOPIC_STATUS = 0x0001 << 0; [...] const StatusKind
SUBSCRIPTION_MATCHED_STATUS = 0x0001 << 14;"

**Repo:** `crates/cpp/include/dds/core/status/State.hpp::StatusMask`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `StatusMask` with the bit values of the IDL of DDS 1.2
(`data_available() == 1 << 10` etc.), `all()`, `none()` and `get_status<STATUS>()` per
status class.

### §7.5.4 InconsistentTopicStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159 (`dds-1.2.pdf`) — "struct
InconsistentTopicStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TInconsistentTopicStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `InconsistentTopicStatus` has the members of the IDL (total_count,
total_count_change) as accessors; the test builds it from values and reads it.

### §7.5.4 SampleLostStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159 (`dds-1.2.pdf`) — "struct
SampleLostStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `SampleLostStatus` has the members of the IDL (total_count,
total_count_change) as accessors; the test builds it from values and reads it.

### §7.5.4 SampleRejectedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159 (`dds-1.2.pdf`) — "enum
SampleRejectedStatusKind { NOT_REJECTED, REJECTED_BY_INSTANCES_LIMIT,
REJECTED_BY_SAMPLES_LIMIT, REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT }; struct
SampleRejectedStatus { long total_count; long total_count_change;
SampleRejectedStatusKind last_reason; InstanceHandle_t last_instance_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleRejectedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `SampleRejectedStatus` has the members of the IDL (total_count,
total_count_change, last_reason, last_instance_handle) as accessors; the test builds it
from values and reads it.

### §7.5.4 LivelinessLostStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159 (`dds-1.2.pdf`) — "struct
LivelinessLostStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TLivelinessLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `LivelinessLostStatus` has the members of the IDL (total_count,
total_count_change) as accessors; the test builds it from values and reads it.

### §7.5.4 LivelinessChangedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 159–160 (`dds-1.2.pdf`) — "struct
LivelinessChangedStatus { long alive_count; long not_alive_count; long
alive_count_change; long not_alive_count_change; InstanceHandle_t
last_publication_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TLivelinessChangedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `LivelinessChangedStatus` has the members of the IDL (alive_count,
not_alive_count, alive_count_change, not_alive_count_change, last_publication_handle) as
accessors; the test builds it from values and reads it.

### §7.5.4 OfferedDeadlineMissedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
OfferedDeadlineMissedStatus { long total_count; long total_count_change;
InstanceHandle_t last_instance_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TOfferedDeadlineMissedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `OfferedDeadlineMissedStatus` has the members of the IDL
(total_count, total_count_change, last_instance_handle) as accessors; the test builds it
from values and reads it.

### §7.5.4 RequestedDeadlineMissedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
RequestedDeadlineMissedStatus { long total_count; long total_count_change;
InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TRequestedDeadlineMissedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `RequestedDeadlineMissedStatus` has the members of the IDL
(total_count, total_count_change, last_instance_handle) as accessors; the test builds it
from values and reads it.

### §7.5.4 OfferedIncompatibleQosStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
QosPolicyCount { QosPolicyId_t policy_id; long count; }; struct
OfferedIncompatibleQosStatus { long total_count; long total_count_change;
QosPolicyId_t last_policy_id; QosPolicyCountSeq policies; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TOfferedIncompatibleQosStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `OfferedIncompatibleQosStatus` has the members of the IDL
(total_count, total_count_change, last_policy_id, policies) as accessors; the test
builds it from values and reads it.

### §7.5.4 RequestedIncompatibleQosStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
RequestedIncompatibleQosStatus { long total_count; long total_count_change;
QosPolicyId_t last_policy_id; QosPolicyCountSeq policies; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TRequestedIncompatibleQosStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `RequestedIncompatibleQosStatus` has the members of the IDL
(total_count, total_count_change, last_policy_id, policies) as accessors; the test
builds it from values and reads it.

### §7.5.4 PublicationMatchedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
PublicationMatchedStatus { long total_count; long total_count_change; long
current_count; long current_count_change; InstanceHandle_t
last_subscription_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TPublicationMatchedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `PublicationMatchedStatus` has the members of the IDL (total_count,
total_count_change, current_count, current_count_change, last_subscription_handle) as
accessors; the test builds it from values and reads it.

### §7.5.4 SubscriptionMatchedStatus

**Spec:** §7.5.4, p. 23; [DDS] 1.2 IDL, p. 160–161 (`dds-1.2.pdf`) — "struct
SubscriptionMatchedStatus { long total_count; long total_count_change; long
current_count; long current_count_change; InstanceHandle_t
last_publication_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSubscriptionMatchedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `SubscriptionMatchedStatus` has the members of the IDL
(total_count, total_count_change, current_count, current_count_change,
last_publication_handle) as accessors; the test builds it from values and reads it.

---

## §7.5.5 Error Codes

### §7.5.5 Table 7.3: RETCODE_OK

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_OK | Normal return; no exception"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_OK` is no exception: `check_status` throws nothing;
`NO_DATA` is an empty sequence on reading.

### §7.5.5 Table 7.3: RETCODE_NO_DATA

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_NO_DATA | An informational state
attached to a normal return; no exception"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_NO_DATA` is no exception: `check_status` throws nothing;
`NO_DATA` is an empty sequence on reading.

### §7.5.5 Table 7.3: RETCODE_ERROR

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_ERROR | Error | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::Error`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ERROR` becomes `dds::core::Error`, derived from
`dds::core::Exception` and `std::exception` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`). The PDF names `std::logic_error` as the parent
of `Error`, the OMG headers `std::exception`; ZeroDDS follows the headers.

### §7.5.5 Table 7.3: RETCODE_BAD_PARAMETER

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_BAD_PARAMETER |
InvalidArgumentError | std::invalid_argument"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::InvalidArgumentError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_BAD_PARAMETER` becomes `dds::core::InvalidArgumentError`,
derived from `dds::core::Exception` and `std::invalid_argument` (`static_assert` and
throw in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_TIMEOUT

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_TIMEOUT | TimeoutError |
std::runtime_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::TimeoutError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_TIMEOUT` becomes `dds::core::TimeoutError`, derived from
`dds::core::Exception` and `std::runtime_error` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_UNSUPPORTED

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_UNSUPPORTED | UnsupportedError |
std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::UnsupportedError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_UNSUPPORTED` becomes `dds::core::UnsupportedError`,
derived from `dds::core::Exception` and `std::logic_error` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_ALREADY_DELETED

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_ALREADY_DELETED |
AlreadyClosedError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::AlreadyClosedError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ALREADY_DELETED` becomes `dds::core::AlreadyClosedError`,
derived from `dds::core::Exception` and `std::logic_error` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_ILLEGAL_OPERATION

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_ILLEGAL_OPERATION |
IllegalOperationError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::IllegalOperationError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ILLEGAL_OPERATION` becomes
`dds::core::IllegalOperationError`, derived from `dds::core::Exception` and
`std::logic_error` (`static_assert` and throw in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_NOT_ENABLED

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_NOT_ENABLED | NotEnabledError |
std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::NotEnabledError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_NOT_ENABLED` becomes `dds::core::NotEnabledError`, derived
from `dds::core::Exception` and `std::logic_error` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_PRECONDITION_NOT_MET

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_PRECONDITION_NOT_MET |
PreconditionNotMetError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::PreconditionNotMetError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_PRECONDITION_NOT_MET` becomes
`dds::core::PreconditionNotMetError`, derived from `dds::core::Exception` and
`std::logic_error` (`static_assert` and throw in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_IMMUTABLE_POLICY

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_IMMUTABLE_POLICY |
ImmutablePolicyError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::ImmutablePolicyError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_IMMUTABLE_POLICY` becomes
`dds::core::ImmutablePolicyError`, derived from `dds::core::Exception` and
`std::logic_error` (`static_assert` and throw in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_INCONSISTENT_POLICY

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_INCONSISTENT_POLICY |
InconsistentPolicyError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::InconsistentPolicyError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_INCONSISTENT_POLICY` becomes
`dds::core::InconsistentPolicyError`, derived from `dds::core::Exception` and
`std::logic_error` (`static_assert` and throw in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_OUT_OF_RESOURCES

**Spec:** §7.5.5, p. 24, Table 7.3 — "RETCODE_OUT_OF_RESOURCES |
OutOfResourcesError | std::runtime_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::OutOfResourcesError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_OUT_OF_RESOURCES` becomes
`dds::core::OutOfResourcesError`, derived from `dds::core::Exception` and
`std::runtime_error` (`static_assert` and throw in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: parent classes of the exceptions

**Spec:** §7.5.5, p. 24, Table 7.3 — "Std C++ Parent Exception: std::logic_error
(ERROR, UNSUPPORTED, ALREADY_DELETED, ILLEGAL_OPERATION, NOT_ENABLED,
PRECONDITION_NOT_MET, IMMUTABLE_POLICY, INCONSISTENT_POLICY),
std::invalid_argument (BAD_PARAMETER), std::runtime_error (TIMEOUT,
OUT_OF_RESOURCES)"

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — The parent classes match the table (`static_assert`), with the
exception of `Error` (see RETCODE_ERROR).

### §7.5.5 Exceptions in `dds::core` with base `Exception`

**Spec:** §7.5.5, p. 24 — "The DDS-PSM-Cxx maps error codes to C++ exceptions
defined in the dds::core namespace and inheriting from a base Exception class
and the appropriate standard C++ exception."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp::Exception`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Every exception is in `dds::core` and derives from
`dds::core::Exception` and from the standard exception.

### §7.5.5 Exceptions have value semantics

**Spec:** §7.5.5, p. 24 — "Exceptions have value semantics, this means have to
always have deep copy semantics."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Copy constructor and assignment copy the message deeply; the copy
outlives the original.

### §7.5.5 All exceptions in `dds/core/Exceptions.hpp`

**Spec:** §7.5.5, p. 24 — "The full list of exceptions is included in the file
dds/core/Exceptions.hpp."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — All exceptions are in one file. The PDF names it
`dds/core/Exceptions.hpp`, the OMG headers `dds/core/Exception.hpp`; ZeroDDS follows the
headers.

---

## §7.5.6 Time and Duration

### §7.5.6 Time_t and Duration_t as value types Time and Duration

**Spec:** §7.5.6, p. 24 — "This PSM maps the DDS Time_t and Duration_t types
into the value types Time and Duration respectively."

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `dds::core::Time` and `dds::core::Duration`.

### §7.5.6 Seconds and nanoseconds through accessors and mutators

**Spec:** §7.5.6, p. 24 — "In addition to providing their seconds and
nanoseconds state through accessor and mutator methods, these classes provide a
small number of convenience operations:"

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `sec()` and `nanosec()` as getters and setters; a setter for
`nanosec` above 999 999 999 normalizes into the seconds.

### §7.5.6 Mutators of Time and Duration

**Spec:** §7.5.6, p. 24 — "providing their seconds and nanoseconds state through
accessor and mutator methods"

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::sec`,
`crates/cpp/include/dds/core/Duration.hpp::Duration::nanosec`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time::sec(int64_t)`, `Time::nanosec(uint32_t)`,
`Duration::sec(int32_t)`, `Duration::nanosec(uint32_t)`; the test checks the
normalization of `Duration::nanosec`.

### §7.5.6 Increment Time by durations

**Spec:** §7.5.6, p. 24 — "• Time object scan be incremented by durations
expressed as seconds, nanoseconds, milliseconds, or Duration objects."

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::operator+=`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time += Duration`, `Time + Duration`, `Time - Duration`.

### §7.5.6 Time from and to milliseconds

**Spec:** §7.5.6, p. 24 — "• Time object scan be converted to and from times
expressed in milliseconds (or other units) as integer types."

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::from_millisecs`,
`crates/cpp/include/dds/core/Time.hpp::Time::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time::from_millisecs`, `from_microsecs`, `from_secs` and
`to_millisecs`, `to_microsecs`, `to_secs`.

### §7.5.6 Increment Duration by durations

**Spec:** §7.5.6, p. 24 — "• Duration objects can be incremented by durations
expressed as seconds, nanoseconds, milliseconds, or Duration objects."

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::operator+=`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Duration += Duration`, `+`, `-`, `*`, `/`; an infinite operand
makes the sum infinite.

### §7.5.6 Duration from and to milliseconds

**Spec:** §7.5.6, p. 24 — "• Duration objects can be converted to and from
durations expressed in milliseconds(or other units) as integer types."

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::from_millisecs`,
`crates/cpp/include/dds/core/Duration.hpp::Duration::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Duration::from_millisecs` (a value past `int32_t` seconds is
`std::overflow_error`), `from_microsecs`, `from_secs` and `to_millisecs`,
`to_microsecs`, `to_secs`.

### §7.5.6 Duration to milliseconds

**Spec:** §7.5.6, p. 24 — "converted to and from durations expressed in
milliseconds(or other units) as integer types"

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `to_millisecs()` and `to_microsecs()` return the duration as an
integer (`Duration(2, 500000000).to_millisecs() == 2500`); `wait_for_acknowledgments`
and `wait_for_matched` use them.

---

## §7.6 QoS Packages

### §7.6 Extensible policies and QoS without changing the API

**Spec:** §7.6, p. 25 — "The QoS package provides all definitions for Policy and
QoS.The DDS-PSM-Cxx provide extensible policy and extensible QoS. This means
that vendor can easily add additional attributes to policy as well as new
policies to Qos. All of this without requiring changes in the public API."

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`, `crates/cpp/include/dds/core/Value.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Policies are templates over a delegate (`THistory<D>` …), QoS
classes sets of policies (`TEntityQos<PolicySet<…>>`); extensions live in the delegate
(`history->…`, `history.delegate()`).

### §7.6 Extensions through `operator->` and `delegate()`

**Spec:** §7.6, p. 25 — "As explained above, the PSM uses the “operator ->”, or
equivalently the “delegate()” method to access vendor-specific extensions."

**Repo:** `crates/cpp/include/dds/core/Value.hpp::Value::operator->`,
`crates/cpp/include/dds/core/Reference.hpp::Reference::operator->`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `operator->` and `delegate()` on value types (`history->depth()`)
and on reference types (`dp->find_topic(…)`).

### §7.6 Namespaces of the QoS classes

**Spec:** §7.6.2.1, p. 27; §7.13.1, p. 30; §7.2, p. 15, Figure 7.1 —
"dds::domain::qos::DomainParticipantQos [...] dds::topic::qos::TopicQos [...]
dds::sub::qos::SubscriberQos [...] dds::sub::qos::DataReaderQos [...]
dds::pub::qos::PublisherQos [...] dds::pub::qos::DataWriterQos"

**Repo:** `crates/cpp/include/dds/domain/qos/`, `crates/cpp/include/dds/topic/qos/`,
`crates/cpp/include/dds/pub/qos/`, `crates/cpp/include/dds/sub/qos/`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — `dds::domain::qos::DomainParticipantQos`,
`dds::topic::qos::TopicQos`, `dds::pub::qos::{PublisherQos, DataWriterQos}`,
`dds::sub::qos::{SubscriberQos, DataReaderQos}`.

### §7.6 DomainParticipantFactoryQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantFactoryQos { EntityFactoryQosPolicy entity_factory; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The OMG headers have neither
`DomainParticipantFactoryQos` nor a factory (see "DomainParticipantFactory: get_qos and
set_qos"); ZeroDDS has no such class either (an extension in `dds::` would not be allowed by §2.2).

### §7.6 DomainParticipantQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantQos { UserDataQosPolicy user_data; EntityFactoryQosPolicy
entity_factory; };"

**Repo:** `crates/cpp/include/dds/domain/qos/DomainParticipantQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DomainParticipantQos` contains `UserData` and `EntityFactory`, as
the IDL has it.

### §7.6 TopicQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 167–168 (`dds-1.2.pdf`) — "struct
TopicQos { topic_data; durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; ownership; };"

**Repo:** `crates/cpp/include/dds/topic/qos/TopicQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — `TopicQos` contains the 13 policies of the IDL; the test checks the
defaults of RELIABILITY and RESOURCE_LIMITS.

### §7.6 PublisherQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 168 (`dds-1.2.pdf`) — "struct
PublisherQos { PresentationQosPolicy presentation; PartitionQosPolicy partition;
GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory; };"

**Repo:** `crates/cpp/include/dds/pub/qos/PublisherQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `PublisherQos` contains Presentation, Partition, GroupData and
EntityFactory, as the IDL has it.

### §7.6 SubscriberQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 169 (`dds-1.2.pdf`) — "struct
SubscriberQos { PresentationQosPolicy presentation; PartitionQosPolicy
partition; GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory;
};"

**Repo:** `crates/cpp/include/dds/sub/qos/SubscriberQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `SubscriberQos` contains Presentation, Partition, GroupData and
EntityFactory, as the IDL has it.

### §7.6 DataWriterQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 168 (`dds-1.2.pdf`) — "struct
DataWriterQos { durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; user_data; ownership; ownership_strength;
writer_data_lifecycle; };"

**Repo:** `crates/cpp/include/dds/pub/qos/DataWriterQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DataWriterQos` contains the 15 policies of the IDL, RELIABILITY
defaults to RELIABLE; a QoS changes with `qos(q)` on a running writer.

### §7.6 DataReaderQos

**Spec:** §7.6, p. 25; [DDS] 1.2 IDL, p. 168–169 (`dds-1.2.pdf`) — "struct
DataReaderQos { durability; deadline; latency_budget; liveliness; reliability;
destination_order; history; resource_limits; user_data; ownership;
time_based_filter; reader_data_lifecycle; };"

**Repo:** `crates/cpp/include/dds/sub/qos/DataReaderQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DataReaderQos` contains the 12 policies of the IDL, RELIABILITY
defaults to BEST_EFFORT, HISTORY to `KEEP_LAST` with `depth` 1.

---

## §7.6.1 Policy Classes

### §7.6.1 Policies per §7.4 without a common base

**Spec:** §7.6.1, p. 25 — "The DDS-PSM-Cxx mapping for the policy classes as
defined in the DDS v1.2 specification is obtained by applying the generic
mapping rules described in 7.4 with the following guidelines: • the inheritance
from the root Policy class has been ignored"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — The policy classes have no common base class.

### §7.6.1 Names without "QosPolicy"

**Spec:** §7.6.1, p. 25 — "• the trailing “QosPolicy” has to be discarded from
the name as redundant."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — The names are `UserData`, `History`, … without `QosPolicy`.

### §7.6.1 Kind as an enumeration with helper functions per variant

**Spec:** §7.6.1, p. 25–26 — "• Policy kind is represented with a C++
enumeration and an associated constructor type as shown in the example below.
[...] first it associates an enumeration with the Policy defining a code for the
variation (as it was done in the IDL PSM), then, it defines a set of helper
methods to construct the possible variants."

**Repo:** `crates/cpp/include/dds/core/policy/PolicyKind.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Each kind is a `safe_enum` (`HistoryKind`, `ReliabilityKind`, …)
with a static helper per variant (`History::KeepAll()`, `Reliability::BestEffort()`,
`Durability::Persistent()`, `Liveliness::ManualByTopic(d)`, `Ownership::Exclusive()`,
`DestinationOrder::SourceTimestamp()`, `Presentation::TopicAccessScope(…)`).

### §7.6.1 Missing helper functions and `Reliable_`

**Spec:** §7.6.1, p. 26; §7.8, p. 28; §7.13.1, p. 30 — "tqos <<
Reliability::Reliable() << Ownership::Exclusive();"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::TReliability::Reliable`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `tqos << Reliability::Reliable() << Ownership::Exclusive()`
(example of the specification) compiles and works; `Reliable()` has a parameter for the
maximum blocking time.

### §7.6.1 Policies in the namespace `dds::qos`

**Spec:** §7.6.1, p. 25 — "Policy classes are part of the dds::qos namespace"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — The PDF names `dds::qos`, the OMG headers `dds::core::policy`;
ZeroDDS follows the headers (`dds::core::policy`).

### §7.6.1 Name and ID through traits

**Spec:** §7.6.1, p. 25 — "the Policy Name and Policy ID are to be provided by
specialization of the following trait classes:"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_id<P>::value` and `policy_name<P>::name()` are defined for
all 22 policies (ID by DDS 1.2); `OMG_DDS_POLICY_TRAITS` and
`OMG_DDS_DEFINE_POLICY_TRAITS` are available for policies of one's own.

### §7.6.1 Trait `policy_id` with `id = -1`

**Spec:** §7.6.1, p. 25 — "namespace dds { namespace qos { template <typename
Policy> class policy_id { public: enum { id = -1 }; };"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_id` is a class template, specialized per policy with
`value`. The PDF writes `enum { id = -1 }` in the primary template, the OMG headers an
empty template with `value` in the specializations; ZeroDDS follows the headers.

### §7.6.1 Trait `policy_name`

**Spec:** §7.6.1, p. 25 — "template <typename Policy> class policy_name { }; }
}"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_name<P>::name()` returns the policy's name.

### §7.6.1 `HistoryKind_def` and `HistoryKind` as `safe_enum`

**Spec:** §7.6.1, p. 25–26 — "namespace dds { namespace qos { struct
HistoryKind_def { enum Type { KEEP_LAST, KEEP_ALL }; }; typedef
dds::core::safe_enum<HistoryKind_def> HistoryKind; }}"

**Repo:** `crates/cpp/include/dds/core/policy/PolicyKind.hpp::HistoryKind_def`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `HistoryKind_def { enum type { KEEP_LAST, KEEP_ALL }; }` and
`safe_enum<HistoryKind_def> HistoryKind`; they are in `dds::core::policy`.

### §7.6.1 `THistory` as a template over `Value<D>`

**Spec:** §7.6.1, p. 26 — "namespace dds { namespace qos { template <typename D>
class THistory : public dds::core::Value<D> {"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `THistory<D> : Value<D>`.

### §7.6.1 `THistory`: constructors

**Spec:** §7.6.1, p. 26 — "THistory(); THistory(HistoryKind kind, int32_t
depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::THistory`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `THistory()` and `THistory(kind, depth)`.

### §7.6.1 `THistory`: access to `kind`

**Spec:** §7.6.1, p. 26 — "HistoryKind::Type kind() const; HistoryKind::Type&
kind(); THistory& kind(HistoryKind kind);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::kind`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `kind() const`, `kind()` as a reference and `kind(value)` returning
the policy.

### §7.6.1 `THistory`: access to `depth`

**Spec:** §7.6.1, p. 26 — "int32_t depth() const; int32_t& depth(); THistory&
depth(int32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::depth`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `depth() const`, `depth()` as a reference and `depth(value)`
returning the policy.

### §7.6.1 `THistory`: getter by reference and setter with return

**Spec:** §7.6.1, p. 26 — "HistoryKind::Type& kind(); THistory& kind(HistoryKind
kind); int32_t& depth(); THistory& depth(int32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The getters as references (`kind()`, `depth()`) and the setters
returning the policy (chainable).

### §7.6.1 `THistory`: `KeepAll()` and `KeepLast(uint32_t)`

**Spec:** §7.6.1, p. 26 — "static History KeepAll(); static History
KeepLast(uint32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepAll`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepLast`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `History::KeepAll()` and `History::KeepLast(depth)`.

### §7.6.1 `History::KeepLast`: type and default of the parameter

**Spec:** §7.6.1, p. 26 — "static History KeepLast(uint32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepLast`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `KeepLast(uint32_t depth)`, as in the specification; the OMG
headers give no default value.

### §7.6.1 All policies in `dds/qos/Policy.hpp`

**Spec:** §7.6.1, p. 26 — "The full set of policies is included in the mandatory
standard headers in the file dds/qos/Policy.hpp."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — The PDF names `dds/qos/Policy.hpp`, the OMG headers
`dds/core/policy/CorePolicy.hpp`; ZeroDDS follows the headers.

---

## §7.6.1 Policies of [DDS] 1.2

### §7.6.1 USER_DATA → `UserData`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 109 (`dds-1.2.pdf`) — "USER_DATA |
A sequence of octets: “value” | [...] The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::UserData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `UserData` with the attributes value;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 TOPIC_DATA → `TopicData`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) — "TOPIC_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TopicData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TopicData` with the attributes value;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 GROUP_DATA → `GroupData`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) — "GROUP_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::GroupData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `GroupData` with the attributes value;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 DURABILITY → `Durability`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) — "DURABILITY
| A “kind”: VOLATILE, TRANSIENT_LOCAL, TRANSIENT, or PERSISTENT"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Durability`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Durability` with the attributes kind;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 DURABILITY_SERVICE → `DurabilityService`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 111 (`dds-1.2.pdf`) —
"DURABILITY_SERVICE | A duration "service_cleanup_delay", a HistoryQosPolicyKind
"history_kind", and three integers: history_depth, max_samples, max_instances,
max_samples_per_instance"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DurabilityService`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `DurabilityService` with the attributes service_cleanup_delay,
history_kind, history_depth, max_samples, max_instances, max_samples_per_instance;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 PRESENTATION → `Presentation`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 111–112 (`dds-1.2.pdf`) —
"PRESENTATION | An “access_scope”: INSTANCE, TOPIC, GROUP And two booleans:
“coherent_access” “ordered_access”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Presentation`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Presentation` with the attributes access_scope, coherent_access,
ordered_access; `test_every_policy_has_its_default_and_its_attributes` checks the
default of the DDS 1.2 table and the attributes.

### §7.6.1 DEADLINE → `Deadline`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 112 (`dds-1.2.pdf`) — "DEADLINE |
A duration “period”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Deadline`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Deadline` with the attributes period;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 LATENCY_BUDGET → `LatencyBudget`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) —
"LATENCY_BUDGET | A duration “duration” | [...] The default value of the
duration is zero"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::LatencyBudget`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `LatencyBudget` with the attributes duration;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 OWNERSHIP → `Ownership`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) — "OWNERSHIP |
A “kind” SHARED EXCLUSIVE"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Ownership`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Ownership` with the attributes kind;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 OWNERSHIP_STRENGTH → `OwnershipStrength`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) — "OWNERSHIP_
STRENGTH | An integer “value”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::OwnershipStrength`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `OwnershipStrength` with the attributes value;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 LIVELINESS → `Liveliness`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 114 (`dds-1.2.pdf`) — "LIVELINESS
| A “kind”: AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC and a duration
“lease_duration”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Liveliness`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Liveliness` with the attributes kind, lease_duration;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 TIME_BASED_FILTER → `TimeBasedFilter`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 115 (`dds-1.2.pdf`) — "TIME_BASED_
FILTER | A duration "minimum_ separation" | [...] default minimum_separation=0"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TimeBasedFilter`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TimeBasedFilter` with the attributes minimum_separation;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 PARTITION → `Partition`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 115 (`dds-1.2.pdf`) — "PARTITION |
A list of strings “name”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Partition`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Partition` with the attributes name;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 RELIABILITY → `Reliability`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 115–116 (`dds-1.2.pdf`) —
"RELIABILITY | A “kind”: RELIABLE, BEST_EFFORT and a duration
“max_blocking_time” | [...] BEST_EFFORT [...] This is the default value for
DataReaders and Topics. [...] RELIABLE [...] the default value for DataWriters.
[...] max_blocking_time=100ms."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Reliability`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `Reliability` with the attributes kind, max_blocking_time;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 TRANSPORT_PRIORITY → `TransportPriority`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) — "TRANSPORT_
PRIORITY | An integer “value”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TransportPriority`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TransportPriority` with the attributes value;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 LIFESPAN → `Lifespan`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) — "LIFESPAN |
A duration “duration”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Lifespan`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Lifespan` with the attributes duration;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 DESTINATION_ORDER → `DestinationOrder`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) —
"DESTINATION_ ORDER | A “kind”: BY_ RECEPTION_ TIMESTAMP, BY_SOURCE_ TIMESTAMP"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DestinationOrder`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `DestinationOrder` with the attributes kind;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 HISTORY → `History`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 117–118 (`dds-1.2.pdf`) — "HISTORY
| A “kind”: KEEP_LAST, KEEP_ALL And an optional integer “depth” | [...]
KEEP_LAST is the default kind. The default value of depth is 1."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::History`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `History` with the attributes kind, depth;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 RESOURCE_LIMITS → `ResourceLimits`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 118–119 (`dds-1.2.pdf`) —
"RESOURCE_ LIMITS | Three integers: max_samples, max_instances,
max_samples_per_instance | [...] By default, LENGTH_UNLIMITED."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::ResourceLimits`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `ResourceLimits` with the attributes max_samples, max_instances,
max_samples_per_instance; `test_every_policy_has_its_default_and_its_attributes` checks
the default of the DDS 1.2 table and the attributes.

### §7.6.1 ENTITY_FACTORY → `EntityFactory`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 119 (`dds-1.2.pdf`) — "ENTITY_
FACTORY | A boolean: “autoenable_ created_ entities” | [...] By default, TRUE."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::EntityFactory`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `EntityFactory` with the attributes autoenable_created_entities;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 WRITER_DATA_LIFECYCLE → `WriterDataLifecycle`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 119–120 (`dds-1.2.pdf`) —
"WRITER_DATA_ LIFECYCLE | A boolean: “autodispose_ unregistered_ instances” |
[...] By default, TRUE."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::WriterDataLifecycle`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `WriterDataLifecycle` with the attributes
autodispose_unregistered_instances;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

### §7.6.1 READER_DATA_LIFECYCLE → `ReaderDataLifecycle`

**Spec:** §7.6.1, p. 25; [DDS] 1.2 §7.1.3, p. 120 (`dds-1.2.pdf`) —
"READER_DATA_ LIFECYCLE | Two durations “autopurge_ nowriter_ samples_delay” and
“autopurge_ disposed_ samples_delay” | [...] By default, infinite."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::ReaderDataLifecycle`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `ReaderDataLifecycle` with the attributes
autopurge_nowriter_samples_delay, autopurge_disposed_samples_delay;
`test_every_policy_has_its_default_and_its_attributes` checks the default of the DDS 1.2
table and the attributes.

---

## §7.6.2 Entity Class

### §7.6.2 Entity as the root of all entities

**Spec:** §7.6.2, p. 26 — "The Entity class is the root for all DDS entities, as
specified in the DDS v1.2 specification."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/core/Entity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `dds::core::Entity` (`TEntity<detail::Entity>`) is the root;
participant, topic, publisher, subscriber, DataWriter, DataReader have `enable`,
`status_changes`, `instance_handle`, `close`, `retain`.

### §7.6.2 Release at zero references or through `close`

**Spec:** §7.6.2, p. 26 — "Since an Entity is a reference type, its resources
are automatically managed by the middleware. Specifically, the resources
associated with the entity will be reclaimed either when the number of live
reference from the user application to the entity drops to zero, or when the
user explicitly invokes the method close."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/core/detail/Retain.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — An entity's resources are released when the last reference goes
(and no listener is set and no `retain()` is active) or `close()` is called.

---

## §7.6.2.1 QoS and Profiles

### §7.6.2.1 QosProvider loads QoS through a URI

**Spec:** §7.6.2.1, p. 26 — "This specification introduces the concept of a
QosProvider to load a QoS configuration from an URI. The URI is used to deduce
both the protocol to be used to access the QoS configuration as well as the
format in which it is expressed."

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp`,
`crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — The URI determines the protocol and the format: `file:`, `data:`
(RFC 2397), `http:`, `https:`; the format is DDS-XML
(`crates/zerodds-c-api/src/qos_provider_ffi.rs`, commits `5061c168b7f`, `9629b1b5a34`).

### §7.6.2.1 At least `file` URIs and XML per [DDS-CCM]

**Spec:** §7.6.2.1, p. 26 — "Implementation of this specification shall support
at very least file URIs and XML format compliant with the QoS-Profile defined in
the DDS for Lightweight CCM specification [DDS-CCM]."

**Repo:** `crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/zerodds-c-api/tests/qos_provider.rs::a_provider_answers_the_qos_of_every_entity_kind_with_inheritance`

**Status:** `done` — `file:` URIs with XML QoS profiles are read, with inheritance
(`base_name`); the document follows DDS-XML 1.0 (elements `domain_participant_qos`,
`*_QOS` names of the enumerations, `<element>` in partitions).

### §7.6.2.1 `TQosProvider<DELEGATE>` in `dds::core::qos`

**Spec:** §7.6.2.1, p. 27 — "template <typename DELEGATE> class
dds::core::qos::TQosProvider : public dds::core::Reference<DELEGATE> {"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider`,
`crates/cpp/include/dds/core/QosProvider.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`

**Status:** `done` — `TQosProvider<DELEGATE> : Reference<DELEGATE>`; the PDF names the
namespace `dds::core::qos`, the OMG headers `dds::core`; ZeroDDS follows the headers.

### §7.6.2.1 Constructors with URI and profile

**Spec:** §7.6.2.1, p. 27 — "explicit TQosProvider(const std::string& uri, const
std::string& profile); explicit TQosProvider(const std::string& uri);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::TQosProvider`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`

**Status:** `done` — `QosProvider(uri, profile)` and `QosProvider(uri)`; a profile the
document lacks is an error on creation.

### §7.6.2.1 `participant_qos()` and `participant_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::domain::qos::DomainParticipantQos
participant_qos(); dds::domain::qos::DomainParticipantQos participant_qos(const
std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::participant_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `participant_qos()` (default profile) and `participant_qos(id)`
(profile `library::profile`) return `DomainParticipantQos`.

### §7.6.2.1 `topic_qos()` and `topic_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::topic::qos::TopicQos topic_qos();
dds::topic::qos::TopicQos topic_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::topic_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `topic_qos()` (default profile) and `topic_qos(id)` (profile
`library::profile`) return `TopicQos`.

### §7.6.2.1 `subscriber_qos()` and `subscriber_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::sub::qos::SubscriberQos subscriber_qos();
dds::sub::qos::SubscriberQos subscriber_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::subscriber_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `subscriber_qos()` (default profile) and `subscriber_qos(id)`
(profile `library::profile`) return `SubscriberQos`.

### §7.6.2.1 `datareader_qos()` and `datareader_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::sub::qos::DataReaderQos datareader_qos();
dds::sub::qos::DataReaderQos datareader_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::datareader_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `datareader_qos()` (default profile) and `datareader_qos(id)`
(profile `library::profile`) return `DataReaderQos`.

### §7.6.2.1 `publisher_qos()` and `publisher_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::pub::qos::PublisherQos publisher_qos();
dds::pub::qos::PublisherQos publisher_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::publisher_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `publisher_qos()` (default profile) and `publisher_qos(id)`
(profile `library::profile`) return `PublisherQos`.

### §7.6.2.1 `datawriter_qos()` and `datawriter_qos(id)`

**Spec:** §7.6.2.1, p. 27 — "dds::pub::qos::DataWriterQos datawriter_qos();
dds::pub::qos::DataWriterQos datawriter_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::datawriter_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `datawriter_qos()` (default profile) and `datawriter_qos(id)`
(profile `library::profile`) return `DataWriterQos`.

### §7.6.2.1 Example of QosProvider

**Spec:** §7.6.2.1, p. 27–28 — "Below a non mandatory example showing how the
QosProvider can be used: dds::core::qos::QosProvider
qos_provider("file:///smwr/hdd/config-qos.xml", "myprofile");
DataReader<ShapeType> dr(sub, topic, qos_provider.datareader_qos());"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example of the specification; `test_qos_provider`
makes a writer with the provider's QoS of it.

---

## §7.7 Domain Package

### §7.7 Classes of the domain package

**Spec:** §7.7, p. 28 — "The domain package defines the
DomainParticipantFactory, DomainParticipant, and DomainParticipantListener."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`,
`crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_domain_participant_listener`

**Status:** `done` — `DomainParticipant` and `DomainParticipantListener` are in
`dds::domain`. The OMG headers have no `DomainParticipantFactory` (see
"DomainParticipantFactory: get_instance"); by §2 the headers prevail over the PDF.

### §7.7 Complete reference in the headers

**Spec:** §7.7, p. 28 — "For a complete reference see the standard header
files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to the headers; they are compared with the
OMG headers (see §2).

---

## §7.8 Topic Package

### §7.8 Classes of the topic package

**Spec:** §7.8, p. 28 — "The topic packaged defines the classes related to topic
management. As such it provides definitions for the Topic, TopicDescription,
ContentFilteredTopic, MultiTopic, and the TopicListener."

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp`,
`crates/cpp/include/dds/topic/TopicDescription.hpp`,
`crates/cpp/include/dds/topic/ContentFilteredTopic.hpp`,
`crates/cpp/include/dds/topic/MultiTopic.hpp`,
`crates/cpp/include/dds/topic/TopicListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_multitopic_joins_two_generated_topics_for_a_data_reader`

**Status:** `done` — `Topic`, `TopicDescription`, `ContentFilteredTopic`, `MultiTopic` and `TopicListener` are in `dds::topic`.

### §7.8 Topic parameterized by the type, registers the type itself

**Spec:** §7.8, p. 28 — "The topic class is parameterized in the topic type and
transparently performs the registration of type support."

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp::Topic`,
`crates/cpp/include/dds/topic/TopicTraits.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `Topic<T>` registers the type on creation through
`topic_type_support<T>`.

### §7.8 Create a topic

**Spec:** §7.8, p. 28 — "DomainParticipant dp(domainId);
dds::topic::Topic<RadarTrack> topic(dp, "RadarTrackTopic");"

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp::Topic`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `DomainParticipant dp(domainId); Topic<RadarTrack> topic(dp,
"RadarTrackTopic");` compiles and runs (`generated_psm_roundtrip.cpp` with the type
`RadarTrack` of §8.2).

### §7.8 Create a topic with QoS

**Spec:** §7.8, p. 28 — "dds::qos::TopicQos tqos = dp.default_topic_qos(); tqos
<< Reliability::Reliable() << Ownership::Exclusive();
dds::topic::Topic<RadarTrack> topic(dp, "RadarTrackTopic", tqos);"

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp::Topic`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_topic_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `TopicQos tqos = dp.default_topic_qos(); tqos <<
Reliability::Reliable() << Ownership::Exclusive(); Topic<…> topic(dp, name, tqos);`
compiles and works. The PDF writes `dds::qos::TopicQos`, the OMG headers
`dds::topic::qos::TopicQos`.

---

## §7.9 Pub Package

### §7.9 Classes of the pub package

**Spec:** §7.9, p. 28 — "The publication (pub) package defines all the classes
associated with the production of data. As such, it defines the Publisher, the
DataWriter and their associated listeners as well as any types."

**Repo:** `crates/cpp/include/dds/pub/Publisher.hpp`,
`crates/cpp/include/dds/pub/DataWriter.hpp`,
`crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/pub/PublisherListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `Publisher`, `DataWriter`, `AnyDataWriter` and their listeners are
in `dds::pub`.

### §7.9 Mandatory classes in the headers

**Spec:** §7.9, p. 28 — "The mandatory classes are specified in the standard
header files. Below, we focus on the specifics of the DataWriter class."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to the headers (see §2).

### §7.9.1 DataWriter parameterized by delegate and type

**Spec:** §7.9.1, p. 28 — "The DataWriter class is parameterized with respect to
the delegate and the topic type that it writes."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `DataWriter<T, DELEGATE>` has, as the OMG headers do, `template <typename T, template <typename Q> class DELEGATE = detail::DataWriterDelegate>` (and `DataReader<T, DELEGATE>` likewise); `DataWriter<T>` is `DataWriter<T, detail::DataWriterDelegate>` and `DELEGATE_T` is `DELEGATE<T>` (`static_assert` in `test_reference_types`).

### §7.9.1 Overloaded `write` operations for single samples

**Spec:** §7.9.1, p. 28 — "The class provides several different overloaded
methods for writing data by providing single samples"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(sample)`, `write(sample, time)`, `write(sample, handle[,
time])`, `write(TopicInstance[, time])` and `operator<<` (sample, sample with time,
sample with handle).

### §7.9.1 Writing through iterators

**Spec:** §7.9.1, p. 28 — "The class provides several different overloaded
methods for writing data by providing single samples or iterators over samples."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(begin, end[, time])` and `write(samples_begin, samples_end,
handles_begin, handles_end[, time])`.

---

## §7.10 Sub Package

### §7.10 Classes of the sub package

**Spec:** §7.10, p. 28 — "The subscription (sub) package defines all the classes
associated with the consumption of data. As such, it defines the Subscriber, the
DataReader and their associated listeners as well as any types."

**Repo:** `crates/cpp/include/dds/sub/Subscriber.hpp`,
`crates/cpp/include/dds/sub/DataReader.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp`,
`crates/cpp/include/dds/sub/SubscriberListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `Subscriber`, `DataReader`, `AnyDataReader` and their listeners are
in `dds::sub`.

### §7.10 Mandatory classes in the headers

**Spec:** §7.10, p. 28 — "The mandatory classes are specified in the standard
header files. Below, we focus on the specifics of the DataReader class."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to the headers (see §2).

---

## §7.11 Extensible and Dynamic Type Support Package

### §7.11 Annotations and dynamic types

**Spec:** §7.11, p. 29 — "The Extensible and Dynamic Type Support (xtypes)
package defines all the classes associated with the definition of extensible
topics, such as annotations and the definition and manipulation of dynamic
types."

**Repo:** `crates/cpp/include/dds/core/xtypes/Annotations.hpp`,
`crates/cpp/include/dds/core/xtypes/TAnnotation.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_dynamic_types_dynamic_data_and_type_provider`

**Status:** `done` — The annotations of the OMG headers: `Key`, `Optional`, `Shared`,
`Nested`, `Id`, `Final`, `Extensible`, `Mutable`, `Extensibility`, `MustUnderstand`,
`Verbatim`, `Bitset`, `BitBound`; the test checks their kind and values.

### §7.11 Describing, creating and annotating dynamic types

**Spec:** §7.11, p. 29 — "As such, this package introduces all classes necessary
for describing dynamic types and their attributes, creating and annotating
them."

**Repo:** `crates/cpp/include/dds/core/xtypes/TDynamicType.hpp`,
`crates/cpp/include/dds/core/xtypes/TStructType.hpp`,
`crates/cpp/include/dds/core/xtypes/UnionType.hpp`,
`crates/cpp/include/dds/core/xtypes/TCollectionTypes.hpp`,
`crates/cpp/include/dds/core/xtypes/TDynamicData.hpp`,
`crates/cpp/include/dds/core/xtypes/detail/TypeProvider.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_dynamic_types_dynamic_data_and_type_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_bit_masks_union_defaults_and_enum_discriminators`,
`crates/zerodds-c-api/tests/types_load.rs::bitmasks_unions_with_a_default_and_enum_discriminators_are_described`

**Status:** `done` — Types are built from classes (primitive, string, sequence, map, array, structure, union, enumeration, alias, bit set, `BitMaskType`), annotated (`add_annotation`) and changed (`add_member`, `remove_member`); a union has a `default` branch (`UnionCase(member)`, `is_default()`) and can switch on an enumeration (`UnionType<uint32_t>(name, EnumerationType, cases)`); the value of a bit mask in `DynamicData` is the set of the names of the flags that are set. `DynamicData` reads and writes members by name and id, encodes and decodes (XCDR1/2, little/big endian) and goes through JSON; `TypeProvider` loads types from IDL or a DDS-XML type library, bit masks, unions with `default` and unions on an enumeration included; `Topic<DynamicData>` carries samples between writer and reader and answers a query on a field. `fixed` has no value in the XTypes type system (`TypeKind` has none, the C API answers `Unrepresentable`, DDSXTY14-28); a structure with a `fixed` member is not loaded as a DynamicType. `DynamicData::value<T>` takes no sequence and no array; they go through `from_json_text`. `DataRepresentation` and `TypeConsistencyEnforcement` are in no QoS set, as in the OMG headers.

---

## §7.12 C++11 Compatibility

### §7.12 C++03 only, support for some C++11 features

**Spec:** §7.12, p. 29 — "This specification relies on C++03 features only.
However, to improve its efficiency and usability in a C++11 environment, it
provides built-in support for some C++11 features, such as initializer lists."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction to §7.12; each feature has an item. The
headers compile as C++03 (see "§3 [C++] ISO/IEC 14882:2003"); the C++11 features are behind `ZERODDS_CXX11`.

### §7.12 Function `move(LoanedSamples<T>&)`

**Spec:** §7.12, p. 29 — "• A move(LoanedSamples<T>&) function shall be defined
in the same namespace as LoanedSamples<T> that behaves identical to std::move."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::move`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::sub::move(LoanedSamples<T>&)` returns the samples moved; the
test checks that the source is empty afterwards.

### §7.12 `cbegin()` and `cend()` of LoanedSamples and SharedSamples

**Spec:** §7.12, p. 29 — "• LoanedSamples<T> and SharedSamples<T> shall provide
member cbegin() and cend() functions, which return const_iterator irrespective
of the constness of the object."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::LoanedSamples::cbegin`,
`crates/cpp/include/dds/sub/SharedSamples.hpp::SharedSamples::cbegin`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `cbegin()` and `cend()` as members and as namespace-level
functions; they return `const_iterator`.

### §7.12 C++11: LoanedSamples move-only

**Spec:** §7.12, p. 29 — "• LoanedSamples<T> shall be implemented as a
first-class move-only type using move operations. A representative example is
std::uniqe_ptr."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::LoanedSamples`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — Copying is deleted, moving exists (`static_assert` in the test).

### §7.12 C++11: namespace-level `begin()` and `end()`

**Spec:** §7.12, p. 29 — "• LoanedSamples<T> and SharedSamples<T> shall provide
namespace level begin() and end() functions to facilitate use of range-based for
loop."

**Repo:** `crates/cpp/include/dds/core/array.hpp::begin`,
`crates/cpp/include/dds/sub/LoanedSamples.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::core::begin`/`end` and the members for the range-based loop;
the test uses them for `SharedSamples` and `LoanedSamples`.

### §7.12 `dds::core::array` as an alias of `std::array`

**Spec:** §7.12, p. 29 — "• dds::core::array shall be a template typedef to
std::array."

**Repo:** `crates/cpp/include/dds/core/array.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `template <typename T, std::size_t N> using array = std::array<T,
N>;`.

### §7.12 C++11: enumerations with `enum class`

**Spec:** §7.12, p. 29 — "• Enumerations shall use built-in type-safe
enumerations with enum class syntax."

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — The generated enumerations are `enum class`
(`an_enumeration_takes_an_underlying_type_from_its_bit_bound` in
`crates/lang-rules/src/adapter/cpp.rs`); the policy kinds are `safe_enum` by §7.4.3.

### §7.12 C++11: move operations for all value types

**Spec:** §7.12, p. 29 — "• Move operations (move constructor and move assign)
shall be provided for all Value<DELEGATE> types."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `Value<D>` and the value types (policies, QoS sets, samples,
`SampleInfo`, statuses) have a move constructor and move assignment; the test checks
that data go over and the source stays empty.

### §7.12 C++11: generated types with move operations per idl2cpp11

**Spec:** §7.12, p. 29 — "• Plain language binding shall be augmented as follows
• Generated code for complex types shall use move operations (move-assignment,
move-constructor) as defined in idl2cpp11 (ptc/2012-04-03) struct type mapping."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The generated classes (default, `--cpp-mapping psm`) have the implicitly
generated move operations (`static_assert` and a move in the round trip). idl2cpp11
(ptc/2012-04-03) is not in the cache.

### §7.12 C++11: arrays by `const` reference

**Spec:** §7.12, p. 29 — "• Structures containing arrays shall use a
const-reference parameter for arrays as opposed to pass-by-value."

**Repo:** `tools/idlc/src/generate.rs`, `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:**
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — The constructor of the generated class takes an array as `const
::dds::core::array<…>&` (Rust test of the code generator).

### §7.12 C++11: namespace-level and member `swap`

**Spec:** §7.12, p. 29 — "• A namespace level swap(t1) and a member swap shall
be provided for each generated class."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`,
`crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_structure_has_swap_as_a_member_and_in_its_namespace`

**Status:** `done` — Every generated class has `swap` as a member and in its namespace
(round trip and Rust test of the code generator).

### §7.12 C++11: `noexcept` allowed

**Spec:** §7.12, p. 29 — "• Move-assign, move-constructor, and member swap
functions, and namespace-level swap may provide noexcept specification to allow
efficient and exception-safe resizing of standard containers."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission; no requirement.

---

## §7.13 Examples

### §7.13.1 C++03 example

**Spec:** §7.13.1, p. 29–31 — "This sub clause provides an example for full
application writing and reading RadarTracks topics."

**Repo:** `crates/cpp/tests/generated_psm_roundtrip.cpp`,
`crates/cpp/tests/generated/psm_probe.idl`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The application of §7.13.1 (partition, Reliable, Transient,
KeepLast(10), TransportPriority(14), `DataWriterQos dwqos(tqos)`, `write`, `<<`, reading
into a vector of samples) runs over the type `RadarTrack` of §8.2. Inconsistencies of
the specification with itself: the example writes `Samples<RadarTrack>` for
`Sample<RadarTrack>`, `drqos` is not declared, and `RadarTrack track("alpha", 100, 200)`
has three arguments against five in §8.2.

### §7.13.2 C++11 example

**Spec:** §7.13.2, p. 31–32 — "While not requiring C++11 the DDS-PSM-Cxx API
described in this specification has built-in support for some of the most
interesting C++11 features."

**Repo:** `crates/cpp/tests/generated_psm_roundtrip.cpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — The selection
`dr.select().max_samples(100).state(DataState::…).take()` with a range-based loop runs.
The PDF writes `.data(DataState::new_data())` and `samples.data()` in the loop; the OMG
headers have `state(…)`.

---

## §8.1 Type Mapping

### §8.1 Type system per [DDS-XTypes], the rest per its binding

**Spec:** §8.1, p. 33 — "The type system for DDS topic types is defined by the
Extensible and Dynamic Topic Types for DDS specification [DDS-XTypes]. This sub
clause defines the set of rules to be used in order to map abstract DDS topic
types into C++ types that can be used by application programmers. Those aspects
of the DDS Type System that are not addressed below are as specified in the
Plain Language Binding as defined by [DDS- XTypes] (which in turn is defined in
terms of an IDL-to-C++ mapping)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction to §8.1; the rules have an item each.

### §8.1.1 Aggregation types as a class

**Spec:** §8.1.1, p. 33 — "DDS aggregation types shall be mapped to a C++
class."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`, `tools/idlc/src/generate.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — By default (`--cpp-mapping psm`) every structure is a class (`class
Reading`) with private members and accessors.

### §8.1.1 Attributes encapsulated

**Spec:** §8.1.1, p. 33 — "Contained attributes shall be encapsulated."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — The attributes are private members (`id_`, `value_`, …).

### §8.1.1 Accessors per §7.4

**Spec:** §8.1.1, p. 33 — "Accessors shall be provided following the rules
described in 7.4."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — `NT attribute() const`, `NT& attribute()`, `attribute(value)` for
simple types; `const CT&`, `CT&` and setters for constructed types (see the items on
§7.4.6).

### §8.1.1 Internal state unspecified

**Spec:** §8.1.1, p. 33 — "The representation of internal state is unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement of the specification; no requirement.

### §8.1.2 Primitive and collection types per Table 7.1

**Spec:** §8.1.2, p. 33 — "IDL primitive and collection types used to define a
topic type shall be mapped to C++ following the rules listed in Table 7.1."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — All rows of Table 7.1 by default (`--cpp-mapping psm`; `static_assert` per
row); the option `--cpp-mapping native` differs for `map` and `long double` (see the items on Table
7.1).

### §8.1.3 Enumerations with the same name and constants

**Spec:** §8.1.3, p. 33 — "IDL enumerations shall be mapped into C++
enumerations with exactly the same enumeration name and enumeration constants."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — `enum class Colour { RED, GREEN, BLUE }` with the name and the
constants of the IDL; the round trip uses `Colour::GREEN`.

### §8.1.4 `@Optional` → `dds::core::optional<T>`

**Spec:** §8.1.4, p. 33 — "Attributes annotated though the @Optional annotation
are mapped to a template instantiation of the class dds::core::optional<T> with
T equal to the type attribute would normally map as per the rules specified
above."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — An `@optional` attribute is `dds::core::optional<T>` (`maybe()`,
`z()` in `RadarTrack`); presence goes through the codec.

### §8.1.4 `dds::core::optional` and `@Optional`

**Spec:** §8.1.4, p. 33 — "mapped to a template instantiation of the class
dds::core::optional<T>"

**Repo:** `crates/cpp/include/dds/core/Optional.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`test_layout_headers_and_find`

**Status:** `done` — `dds::core::optional<T>` with `is_set()`, `get()`, `set(v)`,
`reset()`.

### §8.1.5 `@Shared` → pointer

**Spec:** §8.1.5, p. 33 — "Attributes annotated through the @Shared annotation
are mapped to a pointer of the type they would normally map as per the rules
specified above."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`,
`crates/idl-lower/src/annotation.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — An attribute with `@Shared` (XTypes 1.0 Beta 1, `@external` in
XTypes 1.3) is `std::shared_ptr<T>` (`plot()` in `RadarTrack`); `@Shared` lowers to
`@external` (`idl-lower`). The PDF's example uses `std::vector<uint8_t>*` in the
constructor and `plot_ref_t` in the accessor, which differ from each other.

### §8.1.5 `@Shared`

**Spec:** §8.1.5, p. 33 — "Attributes annotated through the @Shared annotation"

**Repo:** `crates/idl-lower/src/annotation.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — The name `@Shared` (case-insensitive) is known as an annotation and
is not skipped as an unknown one.

---

## §8.2 Example

### §8.2 Example `RadarTrack`

**Spec:** §8.2, p. 33–34 — "This sub clause provides a simple yet representative
example demonstrating the ISO/IEC mapping for DDS types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example of the specification; `psm_probe.idl` contains
the type `RadarTrack`, `generated_psm_roundtrip.cpp` uses it.

---

## Audit-Status

396 done / 0 partial / 0 open / 54 n/a (informative) / 0 n/a (rejected).

Test run: the results of the cited tests come from the Linux test host.

