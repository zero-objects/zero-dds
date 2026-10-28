# DDS-PSM-Cxx 1.0 (ISO/IEC C++ 2003 Language DDS PSM) — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-psm-cxx-1.0.pdf` (34 Seiten, OMG
formal/2013-11-01, November 2013) —
[OMG DDS-PSM-Cxx 1.0](https://www.omg.org/spec/DDS-PSM-Cxx/1.0/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben ohne weiteren Zusatz sind PDF-Seiten dieser PDF (PDF-Seite =
gedruckte Seite + 10). Jeder normative Satz von §1 bis §8, jede Zeile von
Table 7.1, Table 7.2 und Table 7.3 und der Tabellen von §7.4.5 und §7.4.6 und
jede Operation der Klassen, die die PDF ausschreibt (`safe_enum`,
`SampleLostStatus`, `THistory`, `policy_id`, `policy_name`, `TQosProvider`),
ist ein eigenes Item. §2.1 verlangt alle Programmierschnittstellen der
Konformitätsstufen von [DDS]; [DDS] ist nach §3 die Version 1.2
(`internal/standards/cache/omg/dds-1.2.pdf`, PDF-Seite = gedruckte Seite + 12).
Die Operationen der Klassen von DCPS ([DDS] 1.2 §7.1.2, S. 25–107) und die
eingebauten Topics (§7.1.5, S. 147–150) sind deshalb je ein Item unter §2.1;
§7.5.4 verlangt die Statusklassen von [DDS] (IDL S. 159–161) und §7.6.1 die
Policies ([DDS] 1.2 §7.1.3, S. 108–120), je Klasse ein Item. Operationen, die
sich nur durch `_w_timestamp` unterscheiden, und Paare aus `set_` und `get_`
sind ein Item. `done` steht nur, wo Code und Test die Anforderung abdecken; ein zitierter Test
nennt die Testfunktion als `datei::modul::name`, ein Test in C++ als
`datei::funktion`. Eine Fundstelle im Repo nennt Datei und Symbol, keine
Zeilennummer. Dieses Dokument nennt Tests; ihre Ergebnisse kommen vom
Linux-Testhost, nicht aus diesem Dokument. Wo Code eine Anforderung abbildet und
kein Test sie ganz prüft, ist das Item `partial` und die Notiz nennt, was nicht
geprüft ist.

**Kontext:** DDS-PSM-Cxx 1.0 ist in ZeroDDS über diese Teile verteilt:

- `crates/cpp/` (`zerodds-cpp`) — die handgeschriebenen Header unter
  `crates/cpp/include/dds/` (`dds::core`, `dds::domain`, `dds::topic`,
  `dds::pub`, `dds::sub`), das CMake-Ziel `zerodds-cpp` in
  `crates/cpp/CMakeLists.txt` (nur Header; C++03 und später) und die Tests:
  `crates/cpp/tests/smoke_dds_psm.cpp` (übersetzt und ausgeführt von
  `crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke`, in CMake das
  Ziel `smoke_dds_psm`), `crates/cpp/tests/generated_type_roundtrip.cpp` und
  `crates/cpp/tests/generated_psm_roundtrip.cpp` (übersetzt und ausgeführt von
  `crates/cpp/tests/generated_type_via_cargo.rs`, die zweite mit `--cpp-mapping psm`).
- `crates/zerodds-c-api/` (`libzerodds`, `include/zerodds.h`) — jede
  Operation der Header ruft eine Funktion des C-API auf; die C++-Klassen halten
  als Zustand nur Zeiger auf die Objekte des C-API.
- Der C++-Codegen: `zerodds-idlc generate --lang c++`
  (`tools/idlc/src/generate.rs`) über `crates/lang-rules` (`adapter/cpp.rs`,
  `emit/cpp.rs`) und `crates/lang-render/src/cpp.rs`. Er erzeugt die
  Topic-Typen und je Typ die Spezialisierung
  `dds::topic::topic_type_support<T>`, die `Topic<T>`, `DataWriter<T>` und
  `DataReader<T>` verwenden.

`crates/cpp/include/zerodds/dds.hpp` (`zerodds::Runtime`,
`zerodds::TypedWriter<T>`) ist eine eigene API außerhalb von `dds::` und nicht
Gegenstand dieses Dokuments. Die Abbildung von IDL auf C++ nach IDL4 to C++
steht in [`idl4-cpp-1.0.md`](idl4-cpp-1.0.md), die Endpoint-API in
[`zerodds-endpoint-cpp-1.0.md`](zerodds-endpoint-cpp-1.0.md); hier stehen nur
die Regeln, die DDS-PSM-Cxx selbst an die Typen stellt (§7.4, §8).

Nach §2 sind die PDF und die C++-Header der OMG beide normativ, bei einem
Widerspruch gelten die Header. Die Header
(`http://www.omg.org/spec/DDS-PSM-Cxx/20121110/sources`, 196 Dateien) liegen im
Cache unter `internal/standards/cache/omg/dds-psm-cxx-1.0-sources/`, die
API-Dokumentation nicht. Mit einem Skript verglichen sind die Dateinamen und die
Bezeichner (Klassen, Typedefs, Funktionen, Makros) aller Header außerhalb von
`detail/`; nicht verglichen sind `const`, Rückgabetypen und Ausnahmen der
einzelnen Signaturen. Wo PDF und Header sich unterscheiden, nennt ein Item den
Unterschied und sagt, welchem ZeroDDS folgt. Ein Item unter §2.1 prüft, ob die
C++-API die Operation unter dem Namen der Header anbietet.

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Scope

### §1 Zweck: eine neue C++-API für DDS

**Spec:** §1, S. 11 — "The purpose of this document is to specify the ISO/IEC
C++ PSM for DDS. This new PSM provides a new C++ API for programming DDS which
is clear, simple, expressive, safe, efficient, extensible, and portable."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zielbeschreibung; die Anforderungen stehen ab §2.

### §1 Keine Wirkung auf der Leitung, API aus Headern

**Spec:** §1, S. 11 — "The ISO/IEC-C++ PSM does not impact on-the-wire
interoperability with other language mappings. The PSM API is defined by means
of a set of C++ header files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage über die Spec. Die Header von ZeroDDS haben
keinen eigenen Weg zur Leitung: sie rufen das C-API auf. Die Bytes eines Samples erzeugt
`topic_type_support<T>::encode` (vom Codegen erzeugt) oder, für `dds::core::ByteSeq` und
`std::string`, die Spezialisierung in `crates/cpp/include/dds/topic/TopicTraits.hpp`,
die die Bytes unverändert übergibt.

### §1 Umfang: alle Profile von DCPS, XTypes, QoS-Profile, ohne DLRL

**Spec:** §1, S. 11 — "This PSM includes all DCPS conformance profiles defined
in the DDS specification. In addition, it includes platform-specific mappings
for: • The programming interface specified by [DDS-XTypes] • Accessing QoS
profiles such as are specified in [DDS-CCM] This specification only addresses
the DCPS layer of the DDS specification. The optional DLRL layer may be
addressed separately in a future specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Umfang; die Anforderungen dazu stehen in §2.1 (Profile
von DCPS, Stufe Extensible and Dynamic Types, XML-QoS-Profile).

### §1 Neue C++-Abbildung des Typsystems

**Spec:** §1, S. 11 — "This specification also introduces a new C++ mapping for
the DDS type system as specified in the Extensible and Dynamic Topic Types
Specification [REF]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §8. Die PDF schreibt „[REF]“ statt eines
Verweises.

---

## §2 Conformance

### §2 PDF und C++-Header normativ, bei Widerspruch die Header

**Spec:** §2, S. 11; Titelseite, S. 1 — "This specification consists of this
document as well as a set of C++ header files, references on the cover page.
Both are normative. In the event of a conflict between them, the latter shall
prevail."

**Repo:** `crates/cpp/include/dds/dds.hpp`, `crates/cpp/CMakeLists.txt`

**Tests:** `crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own`,
`crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_signatures_follow_the_normative_headers`

**Status:** `done` — Die Header der OMG liegen im Cache
(`internal/standards/cache/omg/dds-psm-cxx-1.0-sources/`, 196 Dateien). Verglichen sind
Dateinamen, Bezeichner (Klassen, Typedefs, Funktionen, Makros von `refmacros.hpp`) und die
Signaturen der Member (Name, Parameterzahl, `const`, Rückgabetyp, Wert oder Referenz;
`tools/psm-signature-diff.py`). Bei Abweichung gelten die Header: Getter der Policies und
Status geben wie in den OMG-Headern Werte zurück, `on_offered_incompatible_qos` nimmt den
Writer als Wert. Die Schreibweisen der OMG-Header, die von den DDS-Namen abweichen
(`offered_deadlined_missed_status`, `offered_incompatible_qos_staus`,
`matched_publications_data`), sind als zweite Namen vorhanden. Jeder öffentliche Header
übersetzt allein (144 Header). Nicht verglichen: Ausnahmespezifikationen und
Default-Argumente; die Unterschiede, die das Werkzeug noch zeigt, sind Makro- und
Schreibweisen des Textvergleichs (gelesen, kein Befund). Das CMake-Ziel
`zerodds_psm_standalone_headers` (Test `the_cmake_target_for_standalone_headers_builds`) ist
`#[ignore]` und in dieser Runde nicht gelaufen.

---

## §2.1 Conformance Profiles

### §2.1 Konformität parallel zu den Stufen von [DDS]

**Spec:** §2.1, S. 11; [DDS] 1.2 §2, S. 14–15 (`dds-1.2.pdf`) — "Conformance to
this specification parallels conformance to the DDS specification itself and
consists of the same conformance levels. For example, an implementation may
conform to the DDS Minimum Profile with respect to this PSM, meaning that all of
the programming interfaces identified by the DDS specification as pertaining to
that conformance level must be implemented in this PSM."

**Repo:** `crates/cpp/include/dds/core/detail/conformance.hpp`,
`crates/cpp/include/dds/dds.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Alle Operationen der Klassen von DCPS sind in den Headern
vorhanden; die Items der folgenden Abschnitte nennen je Operation Code und Test.
`core/detail/conformance.hpp` definiert die Schalter der OMG für Content-subscription, MultiTopic,
Ownership, Persistence und die XTypes-Stufen. ZeroDDS nennt für die C++-API keine Stufe
der Konformität.

### §2.1 Object Model Profile (DLRL) ausgenommen

**Spec:** §2.1, S. 11 — "The one exception to this rule is the Object Model
Profile, which defines the Data Local Reconstruction Layer (DLRL); DLRL is
outside of the scope of this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abgrenzung durch die Spec. Das Object Model Profile
umfasst nach [DDS] 1.2 §2 (S. 15) auch `access_scope = GROUP` von PRESENTATION; die
C++-API kennt den Wert (`PresentationAccessScopeKind::GROUP` in
`crates/cpp/include/dds/core/policy/PolicyKind.hpp`).

### §2.1 Stufe Extensible and Dynamic Types

**Spec:** §2.1, S. 11 — "In addition to the conformance level defined in the DDS
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

**Status:** `done` — `dds::core::xtypes` ist vorhanden (commits `1c10c2b3b74`, `6e1c3aebfb8`): DynamicType, die primitiven, String-, Sequenz-, Map-, Array-, Struktur-, Union-, Enumerations-, Alias-, Bitset- und Bitmask-Typen, Member, Annotationen, DynamicData und TypeProvider; dazu die Policy-Klassen `DataRepresentation` und `TypeConsistencyEnforcement` und die Built-in-Topic-Typen. Einzelheiten und `fixed` im Item „§7.11“.

### §2.1 Erzeugen und QoS aus XML-QoS-Profilen, sonst UNSUPPORTED

**Spec:** §2.1, S. 11 — "This PSM furthermore defines methods to create Entities
and to set their QoS based on the XML QoS libraries and profiles defined by the
DDS for Lightweight CCM specification. Implementations that support these XML
QoS profiles shall implement these operations fully; other implementations shall
indicate failure with the DDS-standard UNSUPPORTED error."

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp`,
`crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `dds::core::QosProvider` liest die QoS-Profile eines
DDS-XML-Dokuments von einem `file:`-, `data:`-, `http:`- oder `https:`-URI (commits
`5061c168b7f`, `9629b1b5a34`). Ein Schema ohne Client ist `UnsupportedError`
(`gopher://…` in `test_qos_provider`). Die QoS erzeugen Entitäten (ein DataWriter mit
der QoS des Providers in `test_qos_provider`).

### §2.1 Plain Language Binding als optionaler Konformitätspunkt

**Spec:** §2.1, S. 11 — "The Plain Language Binding for C++ defined in this
specification represents an optional conformance point. Implementers may support
either this Language Binding or the previously defined Plain Language Binding
for C++ defined in [DDS-XTypes]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis zur Wahl. Der Codegen erzeugt in der Vorgabe (`--cpp-mapping psm`) das Binding von §8 (`zerodds-idlc`, commit `56ff5b5f25f`; Items zu
§8); das Binding von [DDS-XTypes] 1.0 Beta 1 (ptc/2010-05-12) liegt nicht im Cache und
ist nicht verglichen.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: Entity

### §2.1 Entity: set_qos und get_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 25–26 (`dds-1.2.pdf`), Tabelle
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

**Status:** `done` — `qos()` liest, `qos(const Qos&)` setzt die QoS von
DomainParticipant, Publisher, Subscriber, Topic, DataWriter und DataReader; ausserdem
`<<` und `>>` an Publisher, Subscriber, DataWriter und DataReader. Eine veränderbare
Policy ändert sich, eine unveränderbare ist `ImmutablePolicyError`
(`test_qos_of_a_running_entity_is_set_and_read`).

### §2.1 Entity: set_listener und get_listener, Listener beim Erzeugen

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle
Entity — "abstract set_listener (a_listener: Listener, mask: StatusKind []):
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

**Status:** `done` — `listener(l, mask)` setzt, `listener()` liest den Listener jeder
Entitätsart; jeder Konstruktor nimmt Listener und Maske an
(`test_entities_are_created_with_qos_listener_and_mask`). Die Tabelle des C-API ist je
Entität und wird beim Zerstören abgemeldet; jeder Rückruf der Writer-, Reader- und
Participant-Tabellen erreicht seine virtuelle Methode mit der auslösenden Entität
(`test_every_listener_callback_reaches_its_virtual_method`).

### §2.1 Entity: enable

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle
Entity — "enable: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::enable`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `enable()` aller Entitätsarten ruft die Funktionen
`zerodds_*_enable` des C-API; `test_entity_operations` ruft es für Participant, Topic,
Publisher, Subscriber, DataWriter (zweimal) und DataReader auf.

### §2.1 Entity: get_statuscondition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle
Entity — "get_statuscondition: StatusCondition"

**Repo:** `crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — Die PSM hat keine Operation `get_statuscondition`:
`StatusCondition(entity)` erzeugt die Bedingung der Entität (`TStatusCondition`,
Konstruktor aus einer Entität), `entity()` gibt sie zurück, `enabled_statuses` setzt die
Maske.

### §2.1 Entity: get_status_changes

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle
Entity — "get_status_changes: StatusKind []"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::status_changes`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `status_changes()` liefert die ausgelösten Status;
`test_entity_operations` prüft PUBLICATION_MATCHED, SUBSCRIPTION_MATCHED und
DATA_AVAILABLE und dass das Lesen des Status ihn löscht.

### §2.1 Entity: get_instance_handle

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle
Entity — "get_instance_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::instance_handle`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `instance_handle()`; `test_entity_operations` prüft, dass sechs
Entitäten sechs verschiedene, nicht-nil Handles haben und dass der Participant sie
kennt.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DomainParticipantFactory

### §2.1 DomainParticipantFactory: create_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "create_participant (domain_id: DomainId_t, qos_list:
QosPolicy [], a_listener: DomainParticipantListener, mask: StatusKind []):
DomainParticipant"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::TDomainParticipant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`

**Status:** `done` — Die Header der OMG haben keine DomainParticipantFactory:
`DomainParticipant(domain_id[, qos[, listener, mask]]))` erzeugt den Participant
(Konstruktor).

### §2.1 DomainParticipantFactory: delete_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "delete_participant (a_participant:
DomainParticipant): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — Die Header der OMG haben keine Factory: `close()` oder die
Zerstörung der letzten Referenz löscht den Participant
(`test_entities_and_statuses_without_other_coverage`: ein Participant ohne `retain()`
verschwindet mit seiner letzten Referenz).

### §2.1 DomainParticipantFactory: get_instance

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "(static) get_instance: DomainParticipantFactory"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben keine
DomainParticipantFactory und kein `get_instance`; nach §2 gehen die Header der PDF vor.
Das C-API hat `zerodds_dpf_get_instance`, die C++-Klassen rufen es intern.

### §2.1 DomainParticipantFactory: lookup_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "lookup_participant (domain_id: DomainId_t):
DomainParticipant"

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp::dds::domain::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `dds::domain::find(domain_id)` gibt den Participant der Domain oder
einen Null-Participant zurück; ein `retain()`-ter Participant wird gefunden, auch wenn
die Anwendung keine Referenz hält.

### §2.1 DomainParticipantFactory: set_default_participant_qos und get_default_participant_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "set_default_participant_qos (qos_list: QosPolicy
[]): ReturnCode_t; get_default_participant_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_participant_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — Die statische Funktion
`DomainParticipant::default_participant_qos()` liest, `default_participant_qos(qos)`
setzt die Vorgabe der Factory (`test_entities_and_statuses_without_other_coverage` setzt
und stellt sie wieder her).

### §2.1 DomainParticipantFactory: get_qos und set_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "get_qos (out: qos_list: QosPolicy []): ReturnCode_t;
set_qos (qos_list: QosPolicy []): ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben keine
DomainParticipantFactory und keine Operation für ihre QoS; nach §2 gehen die Header der
PDF vor. Eine Klasse `DomainParticipantFactoryQos` gibt es in ZeroDDS nicht.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DomainParticipant

### §2.1 DomainParticipant: create_publisher und delete_publisher

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 34–35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_publisher (qos_list: QosPolicy [], a_listener:
PublisherListener, mask: StatusKind []): Publisher; delete_publisher
(a_publisher: Publisher): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::TPublisher`,
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Publisher(dp[, qos[, listener, mask]])` erzeugt, `close()` oder
die Zerstörung löscht; `delete_contained_entities()` des Participant schließt die
erzeugten Publisher (`test_participant_operations_without_other_coverage`).

### §2.1 DomainParticipant: create_subscriber und delete_subscriber

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_subscriber (qos_list: QosPolicy [], a_listener:
SubscriberListener, mask: StatusKind []): Subscriber; delete_subscriber
(a_subscriber: Subscriber): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::TSubscriber`,
`crates/cpp/include/dds/sub/detail/SubscriberDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Subscriber(dp[, qos[, listener, mask]])` erzeugt, `close()` oder
die Zerstörung löscht; `delete_contained_entities()` des Participant schließt die
erzeugten Subscriber.

### §2.1 DomainParticipant: create_topic und delete_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_topic (topic_name: string, type_name: string,
qos_list: QosPolicy [], a_listener: TopicListener, mask: StatusKind []): Topic;
delete_topic (a_topic: Topic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TTopic.hpp::TTopic::TTopic`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `Topic<T>(dp, name[, type_name][, qos[, listener, mask]])` erzeugt,
`close()` oder die Zerstörung löscht. Der Name des Typs ist der von
`topic_type_support<T>::type_name()` oder der angegebene.

### §2.1 DomainParticipant: create_contentfilteredtopic und delete_contentfilteredtopic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_contentfilteredtopic (name: string, related_topic:
Topic, filter_expression: string, expression_parameters: string []):
ContentFilteredTopic; delete_contentfilteredtopic (a_contentfilteredtopic:
ContentFilteredTopic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TContentFilteredTopic.hpp`,
`crates/cpp/include/dds/topic/detail/ContentFilteredTopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `ContentFilteredTopic<T>(topic, name, Filter)` erzeugt, `close()`
oder die Zerstörung löscht; der Name ist der eigene, `filter_expression()` und
`filter_parameters()` lesen.

### §2.1 DomainParticipant: create_multitopic und delete_multitopic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_multitopic (name: string, type_name: string,
subscription_expression: string, expression_parameters: string []): MultiTopic;
delete_multitopic (a_multitopic: MultiTopic): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/MultiTopic.hpp`,
`crates/cpp/include/dds/topic/TMultiTopic.hpp`,
`crates/cpp/include/dds/topic/detail/MultiTopicDelegate.hpp`,
`crates/zerodds-c-api/src/multitopic_ffi.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_multitopic_joins_two_generated_topics_for_a_data_reader`,
`crates/zerodds-c-api/tests/multitopic_c.rs::a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists`

**Status:** `done` — `MultiTopic<T>(dp, name, expression, params_begin, params_end)` legt die MultiTopic an (`zerodds_dp_create_multitopic`): die Themen der FROM-Klausel sind die Topics des Participant, die ihr Typobjekt kennen; Ausdruck, Themen und Ergebnistyp, die nicht zusammenpassen, ergeben einen Fehler. `close()` löscht sie (`zerodds_dp_delete_multitopic`) und gibt `PreconditionNotMetError`, solange ein Reader auf ihr besteht. Der Test verbindet `Order` und `Customer` zu `Sale` mit `WHERE amount > %0`, ein Sample kommt an.

### §2.1 DomainParticipant: find_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35–36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "find_topic (topic_name: string, timeout: Duration_t):
Topic"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::find_topic`,
`crates/cpp/include/dds/topic/find.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `find_topic(name)` gibt den `AnyTopic` des Namens, für einen
unbekannten Namen einen Null-`AnyTopic`; `dds::topic::find` sucht Topics des Participant
nach Namen.

### §2.1 DomainParticipant: lookup_topicdescription

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "lookup_topicdescription (name: string): TopicDescription"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::lookup_topicdescription`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `lookup_topicdescription(name)` gibt den `AnyTopic` des Namens oder
einen Null-`AnyTopic`.

### §2.1 DomainParticipant: get_builtin_subscriber

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_builtin_subscriber: Subscriber"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_builtin_subscriber`,
`crates/cpp/include/dds/sub/find.hpp::dds::sub::builtin_subscriber`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `dds::sub::builtin_subscriber(dp)` gibt das `BuiltinSubscriber` zurück; `dds::sub::find<DataReader<…BuiltinTopicData>>(builtin, "DCPSParticipant" | "DCPSTopic" | "DCPSPublication" | "DCPSSubscription", it)` liefert die Reader der vier eingebauten Topics (`zerodds_builtin_subscriber_lookup_datareader`); `read()` und `take()` geben ihre Samples mit SampleInfo (`test_builtin_topic_readers`; C-API-Test `builtin_reader`).

### §2.1 DomainParticipant: ignore_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_participant (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::domain::ignore(dp, handle)` und die Form mit Iteratorpaar
rufen `ignore_participant` des C-API (`zerodds_dp_ignore_participant`);
`test_ignore_ends_a_match` prüft, dass der Aufruf mit dem Handle einer entdeckten
Entität das Match mit ihr beendet.

### §2.1 DomainParticipant: ignore_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_topic (handle: InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::topic::ignore(dp, handle)` und die Form mit Iteratorpaar
rufen `ignore_topic` des C-API (`zerodds_dp_ignore_topic`); `test_ignore_ends_a_match`
prüft, dass der Aufruf mit dem Handle einer entdeckten Entität das Match mit ihr
beendet.

### §2.1 DomainParticipant: ignore_publication

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_publication (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::pub::ignore(dp, handle)` und die Form mit Iteratorpaar rufen `ignore_publication` des C-API (`zerodds_dp_ignore_publication`) am Participant des Reader mit dem Handle der entfernten Publication (Handle aus `matched_publications`); `test_ignore_ends_a_match` prüft, dass die Liste der Matches danach leer ist.

### §2.1 DomainParticipant: ignore_subscription

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_subscription (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/domain/discovery.hpp`,
`crates/cpp/include/dds/topic/discovery.hpp`,
`crates/cpp/include/dds/pub/discovery.hpp`, `crates/cpp/include/dds/sub/discovery.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dds::sub::ignore(dp, handle)` und die Form mit Iteratorpaar rufen `ignore_subscription` des C-API (`zerodds_dp_ignore_subscription`) am Participant des Writer mit dem Handle der entfernten Subscription (Handle aus `matched_subscriptions`); `test_ignore_ends_a_match` prüft, dass die Liste der Matches danach leer ist.

### §2.1 DomainParticipant: get_domain_id

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_domain_id: DomainId_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::domain_id`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `domain_id()`.

### §2.1 DomainParticipant: delete_contained_entities

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::delete_contained_entities`,
`crates/cpp/include/dds/core/detail/ChildTracker.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `delete_contained_entities()` schließt die Publisher, Subscriber,
Topics und ContentFilteredTopics, die mit dem Participant erzeugt wurden und die die
Anwendung noch hält, die Writer und Reader zuerst, und ruft danach die Funktion des
C-API. Davor hat das C-API die Kinder freigegeben und ihre Delegates haben sie beim
Zerstören ein zweites Mal freigegeben. Die Operation steht nicht in den Headern der OMG;
sie ist eine Erweiterung von ZeroDDS.

### §2.1 DomainParticipant: assert_liveliness

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::assert_liveliness`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `assert_liveliness()` ruft `zerodds_dp_assert_liveliness`; der Test
prüft den Aufruf ohne Fehler.

### §2.1 DomainParticipant: set_default_publisher_qos und get_default_publisher_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_publisher_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_publisher_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_publisher_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_publisher_qos()` liest, `default_publisher_qos(qos)` setzt
(Rückgabe: der Participant).

### §2.1 DomainParticipant: set_default_subscriber_qos und get_default_subscriber_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_subscriber_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_subscriber_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_subscriber_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_subscriber_qos()` liest, `default_subscriber_qos(qos)`
setzt.

### §2.1 DomainParticipant: set_default_topic_qos und get_default_topic_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_topic_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_topic_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::default_topic_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `default_topic_qos()` liest, `default_topic_qos(qos)` setzt.

### §2.1 DomainParticipant: get_discovered_participants

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_participants (inout: participant_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_participants`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dp->get_discovered_participants()` gibt die Handles der entdeckten
Participants zurück; der Participant des Readers entdeckt den des Writers
(`test_ignore_ends_a_match`).

### §2.1 DomainParticipant: get_discovered_participant_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36–37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_participant_data (inout: participant_data:
ParticipantBuiltinTopicData, participant_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_participant_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_ignore_ends_a_match`

**Status:** `done` — `dp->get_discovered_participant_data(handle)` gibt
`ParticipantBuiltinTopicData` zurück (Test mit dem Handle eines entdeckten Participant);
ein Handle, das keinen entdeckten Participant benennt, ist `PreconditionNotMetError`.

### §2.1 DomainParticipant: get_discovered_topics

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_topics (inout: topic_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topics`,
`crates/cpp/include/dds/topic/discovery.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topics_empty_initially`

**Status:** `done` — `get_discovered_topics()` gibt die Handles zurück; ohne entdeckte
Topics ist die Liste leer.

### §2.1 DomainParticipant: get_discovered_topic_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_topic_data (inout: topic_data:
TopicBuiltinTopicData, topic_handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topic_data`,
`crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`

**Status:** `done` — `get_discovered_topic_data(handle)` gibt `TopicBuiltinTopicData`
zurück; die Klasse trägt jedes Mitglied von DCPSTopic als Policy-Klasse
(`test_topic_data_carries_every_member`, commit `e0b3af9f2c7`).

### §2.1 DomainParticipant: Daten eines entdeckten Topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1.30, S. 44 (`dds-1.2.pdf`) — "This
operation retrieves information on a Topic that has been discovered on the
network."

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`

**Status:** `done` — Die Mitglieder von `TopicBuiltinTopicData` (Name, Typname,
Durability, DurabilityService, Deadline, LatencyBudget, Liveliness, Reliability,
TransportPriority, Lifespan, DestinationOrder, History, ResourceLimits, Ownership,
TopicData) werden aus der Struktur des C-API übernommen; der Test füllt die Struktur und
prüft jedes Mitglied.

### §2.1 DomainParticipant: unbekannter Handle bei get_discovered_topic_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1.30, S. 44 (`dds-1.2.pdf`); §7.5.5,
S. 24 — "The topic_handle must correspond to such a topic. Otherwise, the
operation will fail and return PRECONDITION_NOT_MET."

**Repo:**
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::DomainParticipantDelegate::get_discovered_topic_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`

**Status:** `done` — Ein Handle ohne entdecktes Topic ist `PreconditionNotMetError`
(commit `44aa2082361`).

### §2.1 DomainParticipant: contains_entity

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "contains_entity (a_handle: InstanceHandle_t): boolean"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::contains_entity`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `contains_entity(handle)`; wahr für die sechs Entitäten des Tests,
falsch für sie nach `delete_contained_entities()`.

### §2.1 DomainParticipant: get_current_time

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_current_time (inout: current_time: Time_t):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::current_time`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `current_time()` gibt die Zeit des Participant als `Time` zurück;
der Test prüft, dass es Wanduhrzeit ist.

### §2.1 DomainParticipantListener

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.2.3, S. 47–48 (`dds-1.2.pdf`) — "The
DomainParticipantListener interface extends all other Listener interfaces and
has no additional operation beyond the ones defined by the more general
listeners."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_domain_participant_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `DomainParticipantListener` erbt `PublisherListener`,
`SubscriberListener` und `AnyTopicListener` (Mehrfachvererbung) und hat keine eigene
Operation; `NoOpDomainParticipantListener` ist die Vorgabe. Beide Rückrufe der
Participant-Tabelle (`on_inconsistent_topic`, `on_data_on_readers`) erreichen ihre
virtuelle Methode.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: Topic-Klassen

### §2.1 TopicDescription: get_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "get_participant: DomainParticipant"

**Repo:**
`crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::domain_participant`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `domain_participant()` gibt den Participant des Topic zurück; der
Test prüft die Domain-ID.

### §2.1 TopicDescription: get_type_name

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "readonly: type_name string; get_type_name: string"

**Repo:**
`crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::type_name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `type_name()`; der Name ist der von
`topic_type_support<T>::type_name()` oder der beim Erzeugen angegebene.

### §2.1 TopicDescription: get_name

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "readonly: name string; get_name: string"

**Repo:** `crates/cpp/include/dds/topic/TTopicDescription.hpp::TTopicDescription::name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `name()` für `Topic<T>` und `ContentFilteredTopic<T>`;
`AnyTopicDescription` (`topic/AnyTopicDescription.hpp`) nennt den Namen jeder
Beschreibung.

### §2.1 TopicDescription: Name einer ContentFilteredTopic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.1.3, S. 50 (`dds-1.2.pdf`) — "The name
used to create the TopicDescription."

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::name`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `name()` einer ContentFilteredTopic ist der Name, mit dem sie
erzeugt wurde, nicht der des zugehörigen Topic (commit `fa26fc22a7a`).

### §2.1 Topic: get_inconsistent_topic_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.2, S. 50 (`dds-1.2.pdf`), Tabelle
Topic — "get_inconsistent_topic_status (out: status: InconsistentTopicStatus):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/TTopic.hpp::TTopic::inconsistent_topic_status`,
`crates/cpp/include/dds/topic/AnyTopic.hpp::AnyTopic::inconsistent_topic_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `inconsistent_topic_status()` von `Topic<T>` und `AnyTopic`; ohne
Inkonsistenz ist `total_count()` null.

### §2.1 ContentFilteredTopic: filter_expression

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51–52 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "readonly: filter_expression string"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::filter_expression`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `filter_expression()` gibt den Ausdruck des Filters zurück, mit dem
die ContentFilteredTopic erzeugt wurde.

### §2.1 ContentFilteredTopic: get_related_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "get_related_topic: Topic"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::topic`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `topic()` gibt das verbundene Topic zurück.

### §2.1 ContentFilteredTopic: get_expression_parameters und set_expression_parameters

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51–52 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "get_expression_parameters (out: expression_parameters:
string []): ReturnCode_t; set_expression_parameters (expression_parameters:
string []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/topic/TContentFilteredTopic.hpp::TContentFilteredTopic::filter_parameters`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`

**Status:** `done` — `filter_parameters()` liest, `filter_parameters(begin, end)` setzt;
ein leerer Bereich bei einem Ausdruck mit Parametern ist ein Fehler.

### §2.1 MultiTopic: subscription_expression und Parameter

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.4, S. 52–53 (`dds-1.2.pdf`), Tabelle
MultiTopic — "readonly: subscription_expression string;
get_expression_parameters (out: expression_parameters: string []): ReturnCode_t;
set_expression_parameters (expression_parameters: string []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/topic/MultiTopic.hpp`,
`crates/cpp/include/dds/topic/TMultiTopic.hpp`,
`crates/cpp/include/dds/topic/detail/MultiTopicDelegate.hpp`,
`crates/zerodds-c-api/src/multitopic_ffi.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_multitopic_joins_two_generated_topics_for_a_data_reader`,
`crates/zerodds-c-api/tests/multitopic_c.rs::a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists`

**Status:** `done` — `MultiTopic::expression()` gibt den Ausdruck, `expression_parameters()` die Parameter, `expression_parameters(begin, end)` ersetzt sie (`zerodds_multitopic_set_parameters`); der Test liest sie, ersetzt sie und stellt sie zurück.

### §2.1 TopicListener: on_inconsistent_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.5, S. 53 (`dds-1.2.pdf`), Tabelle
TopicListener — "on_inconsistent_topic (the_topic: Topic, status:
InconsistentTopicStatus): void"

**Repo:** `crates/cpp/include/dds/topic/TopicListener.hpp`,
`crates/cpp/include/dds/topic/AnyTopicListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `TopicListener<T>::on_inconsistent_topic(Topic<T>&, status)` und
`NoOpTopicListener<T>`; der Eintrag der Tabelle des Topic (`zerodds_topic_get_listener`)
ruft die virtuelle Methode mit dem Topic; der Eintrag des Participant ruft
`AnyTopicListener::on_inconsistent_topic` mit einem `AnyTopic`.

### §2.1 TypeSupport: register_type und get_type_name

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.3.6, S. 53–54 (`dds-1.2.pdf`), Tabelle
TypeSupport; §7.8, S. 28 — "register_type (participant: DomainParticipant,
type_name: string): ReturnCode_t; get_type_name: string"

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp::topic_type_support`,
`crates/cpp/include/dds/topic/TTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — Die PSM hat keine Klasse TypeSupport: `Topic<T>` registriert den
Typ beim Erzeugen (`topic_type_support<T>`), `type_name()` nennt ihn;
`Topic<DynamicData>` registriert über `register_type` (`core/xtypes/DynamicTopic.hpp`).

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: Publisher

### §2.1 Publisher: create_datawriter und delete_datawriter

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "create_datawriter (a_topic: Topic, qos: QosPolicy [], a_listener:
DataWriterListener, mask: StatusKind []): DataWriter; delete_datawriter
(a_datawriter: DataWriter): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::DataWriter`,
`crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `DataWriter<T>(pub, topic[, qos[, listener, mask]])` erzeugt,
`close()` oder die Zerstörung löscht.

### §2.1 Publisher: lookup_datawriter

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "lookup_datawriter (topic_name: string): DataWriter"

**Repo:** `crates/cpp/include/dds/pub/find.hpp::dds::pub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::pub::find<DataWriter<T>>(pub, topic_name, …)` gibt die Writer
des Publisher zum Topic-Namen zurück (Iterator-Form und Form mit Höchstzahl), auch als
`AnyDataWriter`; ein unbekannter Name gibt keinen Writer.

### §2.1 Publisher: suspend_publications und resume_publications

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "suspend_publications: ReturnCode_t; resume_publications:
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp::PublisherDelegate::suspend_publications`,
`crates/cpp/include/dds/pub/SuspendedPublication.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `suspend_publications()`, `resume_publications()` und die
RAII-Klasse `SuspendedPublication`; der Test ruft die Klasse auf (Anfang und Ende ohne
Fehler).

### §2.1 Publisher: begin_coherent_changes und end_coherent_changes

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "begin_coherent_changes: ReturnCode_t; end_coherent_changes:
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/detail/PublisherDelegate.hpp::PublisherDelegate::begin_coherent_changes`,
`crates/cpp/include/dds/pub/CoherentSet.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `begin_coherent_changes()`, `end_coherent_changes()` und die
RAII-Klasse `CoherentSet`; der Test ruft die Klasse auf. Die Wirkung beim Reader
(kohärente Zustellung) prüft dieser Test nicht.

### §2.1 Publisher: wait_for_acknowledgments

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::wait_for_acknowledgments`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_acknowledgments(timeout)` wartet auf die Bestätigung der
Samples eines Writers mit passendem Reader und gibt bei Zeitüberschreitung
`TimeoutError`.

### §2.1 Publisher: get_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "get_participant: DomainParticipant"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::participant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `participant()` gibt den Participant des Publisher zurück.

### §2.1 Publisher: delete_contained_entities

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben diese Operation an keiner
Klasse (auch nicht am Participant); nach §2 gehen sie der PDF vor. Das Schließen des
Participant über seine `delete_contained_entities()` (Erweiterung von ZeroDDS) schließt
die Writer jedes Publisher.

### §2.1 Publisher: set_default_datawriter_qos und get_default_datawriter_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57–58 (`dds-1.2.pdf`), Tabelle
Publisher — "set_default_datawriter_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datawriter_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/TPublisher.hpp::TPublisher::default_writer_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `default_writer_qos()` liest, `default_writer_qos(qos)` setzt die
Vorgabe der Writer des Publisher.

### §2.1 Publisher: copy_from_topic_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 58 (`dds-1.2.pdf`), Tabelle
Publisher — "copy_from_topic_qos (inout: a_datawriter_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/qos/DataWriterQos.hpp::DataWriterQos::DataWriterQos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `DataWriterQos(topic_qos)` baut die Writer-QoS aus der Topic-QoS
(Konstruktor aus einer anderen QoS-Art); `test_qos_composition` prüft die Übernahme.

### §2.1 PublisherListener

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.3, S. 71–72 (`dds-1.2.pdf`) —
"PublisherListener: no attributes, no operations (inherits the operations of
DataWriterListener)"

**Repo:** `crates/cpp/include/dds/pub/PublisherListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — `PublisherListener` erbt `AnyDataWriterListener` und hat keine
eigene Operation; `NoOpPublisherListener` ist die Vorgabe. Jeder der vier Einträge der
Tabelle des Publisher (`zerodds_pub_get_listener`) ruft seine virtuelle Methode mit
einem `AnyDataWriter` des auslösenden Writers.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DataWriter

### §2.1 DataWriter: register_instance und register_instance_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "register_instance (instance: Data): InstanceHandle_t;
register_instance_w_timestamp (instance: Data, timestamp: Time_t):
InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::register_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `register_instance(sample)` und `register_instance(sample, time)`
geben das Handle der Instanz des Schlüssels; dieselbe Instanz bei wiederholter
Registrierung. Getestet mit einem erzeugten Typ (`generated_type_roundtrip.cpp`, über
`generated_type_via_cargo`) und mit einem eingebauten Typ mit Schlüssel
(`test_instance_operations_with_a_time_stamp`).

### §2.1 DataWriter: unregister_instance und unregister_instance_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "unregister_instance (instance: Data, handle: InstanceHandle_t):
ReturnCode_t; unregister_instance_w_timestamp (instance: Data, handle:
InstanceHandle_t, timestamp: Time_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::unregister_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `unregister_instance(handle[, time])`.

### §2.1 DataWriter: get_key_value

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `key_value(sample, handle)` und `key_value(TopicInstance, handle)`
geben den Schlüssel einer registrierten Instanz; ein Handle ohne Instanz des Writers ist
eine Ausnahme.

### §2.1 DataWriter: lookup_instance

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::lookup_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `lookup_instance(sample)` gibt das Handle der Instanz des
Schlüssels.

### §2.1 DataWriter: write und write_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "write (data: Data, handle: InstanceHandle_t): ReturnCode_t;
write_w_timestamp (data: Data, handle: InstanceHandle_t, source_timestamp:
Time_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_read_keeps_samples_take_removes_them`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(sample)`, `write(sample, time)`, `operator<<`,
Iteratorbereiche mit und ohne Zeit und `write(TopicInstance)`; der Test prüft, dass jede
Form ihr Sample liefert.

### §2.1 DataWriter: write mit Handle, write_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2.11–12, S. 67–68 (`dds-1.2.pdf`) —
"The special value HANDLE_NIL can be used for the parameter handle. This
indicates that the identity of the instance should be automatically deduced from
the instance_data (by means of the key)."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `write(sample, handle[, time])` schreibt in die benannte Instanz;
ein Handle, das nicht die Instanz des Samples ist, ist `InvalidArgumentError`.

### §2.1 DataWriter: dispose und dispose_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t;
dispose_w_timestamp (data: Data, handle: InstanceHandle_t, timestamp: Time_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::dispose_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `dispose_instance(handle[, time])`; der Reader sieht die Instanz
als NOT_ALIVE_DISPOSED (`generated_type_roundtrip.cpp`).

### §2.1 DataWriter: dispose mit Handle, dispose_w_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::dispose_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `dispose_instance(handle[, time])` nimmt das Handle der Instanz.

### §2.1 DataWriter: wait_for_acknowledgments

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::wait_for_acknowledgments`,
`crates/cpp/include/dds/pub/AnyDataWriter.hpp::AnyDataWriter::wait_for_acknowledgments`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_acknowledgments(timeout)` am DataWriter und am
AnyDataWriter.

### §2.1 DataWriter: get_liveliness_lost_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_liveliness_lost_status (out: status: LivelinessLostStatus):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::liveliness_lost_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `liveliness_lost_status()`; ein Writer ohne Liveliness-Verlust hat
`total_count() == 0`.

### §2.1 DataWriter: get_offered_deadline_missed_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_offered_deadline_missed_status (out: status:
OfferedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::offered_deadline_missed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `offered_deadline_missed_status()` und, unter dem Namen der Header
der OMG, `offered_deadlined_missed_status()`.

### §2.1 DataWriter: get_offered_incompatible_qos_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_offered_incompatible_qos_status (out: status:
OfferedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::offered_incompatible_qos_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `offered_incompatible_qos_status()` und, unter dem Namen der Header
der OMG, `offered_incompatible_qos_staus()`.

### §2.1 DataWriter: get_publication_matched_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_publication_matched_status (out: status:
PublicationMatchedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::publication_matched_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `publication_matched_status()`; der Test prüft `total_count == 1`
und `current_count == 1` für einen Writer und einen Reader.

### §2.1 DataWriter: get_topic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_topic: Topic"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::topic`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `topic()` gibt das Topic des Writers zurück.

### §2.1 DataWriter: get_publisher

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_publisher: Publisher"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::publisher`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `publisher()` gibt den Publisher des Writers zurück.

### §2.1 DataWriter: assert_liveliness

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "assert_liveliness: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::assert_liveliness`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `assert_liveliness()`.

### §2.1 DataWriter: get_matched_subscription_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter; §7.1.2.4.2.23, S. 70 — "get_matched_subscription_data (inout:
subscription_data: SubscriptionBuiltinTopicData, subscription_handle:
InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscription_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — `matched_subscription_data(writer, handle)` gibt die
`SubscriptionBuiltinTopicData` des Readers; ein Handle ohne Reader ist
`InvalidArgumentError`.

### §2.1 DataWriter: get_matched_subscriptions

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_matched_subscriptions (inout: subscription_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscriptions`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `matched_subscriptions(writer)` und die Iterator-Form geben die
Handles der passenden Reader.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DataWriterListener

### §2.1 DataWriterListener: on_liveliness_lost

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_liveliness_lost (the_writer: DataWriter, status:
LivelinessLostStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_liveliness_lost`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft `on_liveliness_lost(writer,
status)` mit dem Writer, der ihn auslöst.

### §2.1 DataWriterListener: on_offered_deadline_missed

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_offered_deadline_missed (the_writer: DataWriter,
status: OfferedDeadlineMissedStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_offered_deadline_missed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_offered_deadline_missed(writer, status)` mit dem auslösenden Writer.

### §2.1 DataWriterListener: on_offered_incompatible_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_offered_incompatible_qos (the_writer: DataWriter,
status: OfferedIncompatibleQosStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_offered_incompatible_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_offered_incompatible_qos(writer, status)` mit dem auslösenden Writer.

### §2.1 DataWriterListener: on_publication_matched

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_publication_matched (the_writer: DataWriter, status:
PublicationMatchedStatus): void"

**Repo:**
`crates/cpp/include/dds/pub/DataWriterListener.hpp::DataWriterListener::on_publication_matched`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`

**Status:** `done` — Der Eintrag der Tabelle ruft `on_publication_matched(writer,
status)` mit dem Writer und seinem `PublicationMatchedStatus`.

### §2.1 Listener: auslösende Entität und Status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72, §7.1.2.5.7, S. 105
(`dds-1.2.pdf`); §7.3, S. 17 — "on_publication_matched (the_writer: DataWriter,
status: PublicationMatchedStatus): void"

**Repo:** `crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp`,
`crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Jeder Rückruf bekommt die Entität, die ihn auslöst, nicht ein
Ersatzobjekt, und ihren Status (commit `24f7d012de2`); die Tabelle ist je Entität und
wird beim Zerstören abgemeldet.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: Subscriber

### §2.1 Subscriber: create_datareader und delete_datareader

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80 (`dds-1.2.pdf`), Tabelle
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
erzeugt, `close()` oder die Zerstörung löscht. Eine ContentFilteredTopic als
Beschreibung ist möglich; ein Filter auf Felder eines Typs ohne Typinformation
(`ByteSeq`) wird abgelehnt, statt alle Samples ungefiltert durchzulassen.

### §2.1 Subscriber: lookup_datareader

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80 (`dds-1.2.pdf`), Tabelle
Subscriber — "lookup_datareader (topic_name: string): DataReader"

**Repo:** `crates/cpp/include/dds/sub/find.hpp::dds::sub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::sub::find<DataReader<T>>(sub, topic_name, …)` gibt die Reader
des Subscriber zum Topic-Namen zurück (auch als `AnyDataReader`).

### §2.1 Subscriber: begin_access und end_access

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80–81 (`dds-1.2.pdf`), Tabelle
Subscriber — "begin_access: ReturnCode_t; end_access: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/CoherentAccess.hpp`,
`crates/cpp/include/dds/sub/detail/SubscriberDelegate.hpp::SubscriberDelegate::begin_access`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Die RAII-Klasse `CoherentAccess(sub)` ruft `begin_access`, `end()`
oder die Zerstörung `end_access`; der Test ruft die Klasse auf.

### §2.1 Subscriber: get_datareaders

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "get_datareaders (out: readers: DataReader [], sample_states:
SampleStateKind [], view_states: ViewStateKind [], instance_states:
InstanceStateKind []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/find.hpp::dds::sub::find`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::sub::find<…>(sub, DataState)` wählt die Reader nach den
Masken der Samples (Sample-, View- und Instance-State), `find` nach Beschreibung und
Name; commit `ea5f1ef12d1`.

### §2.1 Subscriber: notify_datareaders

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "notify_datareaders: ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::notify_datareaders`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `notify_datareaders()`; der Test ruft die Operation auf.

### §2.1 Subscriber: get_participant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "get_participant: DomainParticipant"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::participant`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `participant()`.

### §2.1 Subscriber: delete_contained_entities

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben diese Operation an keiner
Klasse; nach §2 gehen sie der PDF vor. `delete_contained_entities()` des Participant
(Erweiterung von ZeroDDS) schließt die Reader jedes Subscriber.

### §2.1 Subscriber: set_default_datareader_qos und get_default_datareader_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "set_default_datareader_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datareader_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::default_datareader_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `default_datareader_qos()` liest, `default_datareader_qos(qos)`
setzt.

### §2.1 Subscriber: copy_from_topic_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "copy_from_topic_qos (inout: a_datareader_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/qos/DataReaderQos.hpp::DataReaderQos::DataReaderQos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `DataReaderQos(topic_qos)` baut die Reader-QoS aus der Topic-QoS.

### §2.1 SubscriberListener: on_data_on_readers

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.6, S. 104 (`dds-1.2.pdf`), Tabelle
SubscriberListener — "on_data_on_readers (the_subscriber: Subscriber): void"

**Repo:** `crates/cpp/include/dds/sub/SubscriberListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — `SubscriberListener::on_data_on_readers(Subscriber&)` und
`NoOpSubscriberListener`; jeder Eintrag der Tabelle des Subscriber
(`zerodds_sub_get_listener`) ruft seine virtuelle Methode, `on_data_on_readers` mit dem
Subscriber, die übrigen mit einem `AnyDataReader`.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DataReader

### §2.1 DataReader: read

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_read_keeps_samples_take_removes_them`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `read()` und `select()….read()` lassen die Samples im Reader
(commit `903a16b85f0`); Formen mit Vorwärts- und Back-Insert-Iterator, `max_samples`,
der Strom `dr >> read >> samples`.

### §2.1 DataReader: take

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
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

**Status:** `done` — `take()` nimmt die Samples aus dem Reader; ein leerer Reader gibt
eine leere Folge; wirft die Dekodierung, wird der Loan zurückgegeben (commit
`409a44a86e9`).

### §2.1 DataReader: Zustandsmasken bei read und take

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "sample_states: SampleStateKind [], view_states: ViewStateKind [],
instance_states: InstanceStateKind []"

**Repo:** `crates/cpp/include/dds/sub/status/DataState.hpp`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::default_filter_state`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `select().state(DataState)` und `default_filter_state(DataState)`
wählen nach Sample-, View- und Instance-State; die Maskenwerte sind die von DDS 1.4
(`SampleState`, `ViewState`, `InstanceState`, `DataState`); Rust-Tests des C-API prüfen
die Auswertung (`crates/zerodds-c-api/tests/sample_info_fields.rs`).

### §2.1 DataReader: read_w_condition und take_w_condition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read_w_condition (inout: data_values: Data [], inout:
sample_infos: SampleInfo [], max_samples: long, a_condition: ReadCondition):
ReturnCode_t; take_w_condition (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/Query.hpp`,
`crates/cpp/include/dds/sub/cond/QueryCondition.hpp`,
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`,
`crates/zerodds-c-api/tests/query_condition_read.rs::read_and_take_with_a_query_condition_return_only_the_samples_the_expression_selects`

**Status:** `done` — Die PSM hat kein `read_w_condition`: `select().state(…)` entspricht
der ReadCondition, `select().content(query)` der QueryCondition (commit `8733e3630a0`).
Ein Ausdruck auf Feldern wählt nur die passenden Samples
(`generated_type_roundtrip.cpp`: `value > 50`; C-API: `query_condition_read.rs`).

### §2.1 DataReader: read_next_sample und take_next_sample

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_sample (inout: data_value: Data, inout: sample_info:
SampleInfo): ReturnCode_t; take_next_sample (inout: data_value: Data, inout:
sample_info: SampleInfo): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — Die PSM hat keine Operation `read_next_sample`: `read(iterator, 1)`
und `select().max_samples(1)` liefern das nächste Sample; `dr >> max_samples(1) >> take
>> samples` nimmt eines (`test_samples_api`).

### §2.1 DataReader: read_instance und take_instance

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 87 (`dds-1.2.pdf`), Tabelle
DataReader — "read_instance (..., a_handle: InstanceHandle_t, sample_states,
view_states, instance_states): ReturnCode_t; take_instance (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `select().instance(handle).read()` und `.take()` wählen die Samples
einer Instanz (`test_instance_operations_with_a_time_stamp`; mit einem Filter zusammen
in `generated_type_roundtrip.cpp`).

### §2.1 DataReader: read_next_instance und take_next_instance

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 87 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_instance (..., previous_handle: InstanceHandle_t,
sample_states, view_states, instance_states): ReturnCode_t; take_next_instance
(...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `select().next_instance(handle).read()` und `.take()` wählen die
Samples der Instanz nach dem Handle (Handle nil: die erste).

### §2.1 DataReader: read_next_instance_w_condition und take_next_instance_w_condition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_instance_w_condition (..., previous_handle:
InstanceHandle_t, a_condition: ReadCondition): ReturnCode_t;
take_next_instance_w_condition (...): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::Selector`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `select().next_instance(handle).content(query)` verbindet beides
(commit `eb8bfdd601e`); auf einem leeren Reader liefert es nichts.

### §2.1 DataReader: return_loan

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "return_loan (inout: data_values: Data [], inout: sample_infos:
SampleInfo []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp`,
`crates/cpp/include/dds/sub/detail/SampleDetail.hpp::LoanGuard`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_returns_the_loan_when_decode_throws`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — Der Loan wird mit `LoanedSamples` zurückgegeben, auch wenn die
Dekodierung wirft (`LoanGuard`); `return_loan` ist keine Operation der PSM.

### §2.1 DataReader: get_key_value

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `key_value(sample, handle)` und `key_value(handle)` (als
`TopicInstance`) geben den Schlüssel einer Instanz des Readers.

### §2.1 DataReader: lookup_instance

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::lookup_instance`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `lookup_instance(sample)` gibt das Handle der Instanz des
Schlüssels im Reader.

### §2.1 DataReader: create_readcondition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "create_readcondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind []):
ReadCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`,
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `ReadCondition(reader, DataState[, functor])`; `state_filter()` und
`data_reader()` geben Maske und Reader zurück (commit `b221bbd356b`).

### §2.1 DataReader: create_querycondition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "create_querycondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind [],
query_expression: string, query_parameters: string []): QueryCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/QueryCondition.hpp`,
`crates/cpp/include/dds/sub/Query.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `QueryCondition(query, DataState)`; Ausdruck und Parameter werden
gelesen, ersetzt und ergänzt (`expression`, `parameters`, `add_parameter`; commit
`eb8bfdd601e`).

### §2.1 DataReader: delete_readcondition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "delete_readcondition (a_condition: ReadCondition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`,
`crates/cpp/include/dds/sub/cond/detail/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — Die Bedingung wird mit ihrer letzten Referenz gelöscht (kein
`delete_readcondition` in der PSM); der Test erzeugt und zerstört sie.

### §2.1 DataReader: get_liveliness_changed_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_liveliness_changed_status (out: status:
LivelinessChangedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::liveliness_changed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `liveliness_changed_status()`.

### §2.1 DataReader: get_requested_deadline_missed_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_requested_deadline_missed_status (out: status:
RequestedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::requested_deadline_missed_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `requested_deadline_missed_status()`.

### §2.1 DataReader: get_requested_incompatible_qos_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_requested_incompatible_qos_status (out: status:
RequestedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::requested_incompatible_qos_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `requested_incompatible_qos_status()`.

### §2.1 DataReader: get_sample_lost_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_sample_lost_status (out: status: SampleLostStatus):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::sample_lost_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `sample_lost_status()`; ohne Verlust ist `total_count() == 0`.

### §2.1 DataReader: get_sample_rejected_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_sample_rejected_status (out: status: SampleRejectedStatus):
ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::sample_rejected_status`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `sample_rejected_status()`.

### §2.1 DataReader: get_subscription_matched_status

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_subscription_matched_status (out: status:
SubscriptionMatchedStatus): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::subscription_matched_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `subscription_matched_status()`.

### §2.1 DataReader: get_topicdescription

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_topicdescription: TopicDescription"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::topic_description`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `topic_description()`.

### §2.1 DataReader: get_subscriber

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_subscriber: Subscriber"

**Repo:** `crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::subscriber`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `subscriber()`.

### §2.1 DataReader: delete_contained_entities

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "delete_contained_entities: ReturnCode_t"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben diese Operation an keiner
Klasse; nach §2 gehen sie der PDF vor. Die Bedingungen eines Readers sind eigene
Objekte, die mit ihrer letzten Referenz gelöscht werden.

### §2.1 DataReader: wait_for_historical_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "wait_for_historical_data (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::wait_for_historical_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait_for_historical_data(timeout)`; bei Zeitüberschreitung
`TimeoutError`.

### §2.1 DataReader: get_matched_publication_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader; §7.1.2.5.3.33, S. 102 — "get_matched_publication_data (inout:
publication_data: PublicationBuiltinTopicData, publication_handle:
InstanceHandle_t): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/discovery.hpp::dds::sub::matched_publication_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — `matched_publication_data(reader, handle)` und, unter dem Namen der
Header der OMG, `matched_publications_data`.

### §2.1 DataReader: get_matched_publications

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_matched_publications (inout: publication_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/sub/discovery.hpp::dds::sub::matched_publications`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `matched_publications(reader)` und die Iterator-Form.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: SampleInfo

### §2.1 SampleInfo: sample_state

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "sample_state: SampleStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`,
`crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `info.state().sample_state()` gibt den Zustand; die Zustände kommen
aus den Bits des C-API (`SampleDetail.hpp::make_sample_info`); `SampleInfo` ist ein
Werttyp, dessen Teile `test_sample_info_parts_qos_provider_kinds_and_filter_expression`
aus Werten aufbaut und liest.

### §2.1 SampleInfo: view_state

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "view_state: ViewStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`,
`crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `info.state().view_state()` gibt den Zustand; die Zustände kommen
aus den Bits des C-API (`SampleDetail.hpp::make_sample_info`); `SampleInfo` ist ein
Werttyp, dessen Teile `test_sample_info_parts_qos_provider_kinds_and_filter_expression`
aus Werten aufbaut und liest.

### §2.1 SampleInfo: instance_state

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "instance_state: InstanceStateKind"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::state`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `info.state().instance_state()`; ALIVE im Test der Samples,
NOT_ALIVE_DISPOSED nach `dispose_instance` (`generated_type_roundtrip.cpp`).

### §2.1 SampleInfo: disposed_generation_count

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "disposed_generation_count: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::generation_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.generation_count().disposed()`; die Werte setzt
`make_sample_info` aus der Struktur des C-API, die Zählung selbst prüft
`crates/zerodds-c-api/tests/sample_info_fields.rs::disposed_generation_count_increments_on_reactivation`.

### §2.1 SampleInfo: no_writers_generation_count

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "no_writers_generation_count: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::generation_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.generation_count().no_writers()`; der Wert kommt aus der
Struktur des C-API (Rust-Test: `instance_state_unregister_reports_not_alive_no_writers`
in `sample_info_fields.rs`).

### §2.1 SampleInfo: sample_rank

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "sample_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().sample()`; Rust-Test der Zählung:
`sample_rank_counts_down_within_one_batch` in `sample_info_fields.rs`.

### §2.1 SampleInfo: generation_rank

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "generation_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().generation()`.

### §2.1 SampleInfo: absolute_generation_rank

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "absolute_generation_rank: long"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::rank`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.rank().absolute_generation()`.

### §2.1 SampleInfo: source_timestamp

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "source_timestamp: Time_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::timestamp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.timestamp()`; der Zeitstempel eines Writers (`write(sample,
time)`) erreicht die SampleInfo über die Struktur des C-API (Rust-Test:
`crates/zerodds-c-api/tests/write_w_timestamp.rs`).

### §2.1 SampleInfo: instance_handle

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "instance_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::instance_handle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_roundtrip.cpp::main`

**Status:** `done` — `info.instance_handle()`; zwei Samples desselben Schlüssels haben
dasselbe Handle (`generated_type_roundtrip.cpp`).

### §2.1 SampleInfo: publication_handle

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "publication_handle: InstanceHandle_t"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::publication_handle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.publication_handle()`; der Wert kommt aus der Struktur des
C-API.

### §2.1 SampleInfo: valid_data

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "valid_data: boolean"

**Repo:** `crates/cpp/include/dds/sub/SampleInfo.hpp::SampleInfo::valid`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `info.valid()`; wahr für ein Sample mit Daten.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: DataReaderListener

### §2.1 DataReaderListener: on_data_available

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_data_available (the_reader: DataReader): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_data_available`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft `on_data_available(reader)`
mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_sample_rejected

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_sample_rejected (the_reader: DataReader, status:
SampleRejectedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_sample_rejected`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft `on_sample_rejected(reader,
status)` mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_liveliness_changed

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_liveliness_changed (the_reader: DataReader, status:
LivelinessChangedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_liveliness_changed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_liveliness_changed(reader, status)` mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_requested_deadline_missed

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_requested_deadline_missed (the_reader: DataReader,
status: RequestedDeadlineMissedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_requested_deadline_missed`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_requested_deadline_missed(reader, status)` mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_requested_incompatible_qos

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_requested_incompatible_qos (the_reader: DataReader,
status: RequestedIncompatibleQosStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_requested_incompatible_qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_requested_incompatible_qos(reader, status)` mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_subscription_matched

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_subscription_matched (the_reader: DataReader, status:
SubscriptionMatchedStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_subscription_matched`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft
`on_subscription_matched(reader, status)` mit dem auslösenden Reader.

### §2.1 DataReaderListener: on_sample_lost

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_sample_lost (the_reader: DataReader, status:
SampleLostStatus): void"

**Repo:**
`crates/cpp/include/dds/sub/DataReaderListener.hpp::DataReaderListener::on_sample_lost`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_listener_callback_reaches_its_virtual_method`

**Status:** `done` — Der Eintrag der Tabelle des C-API ruft `on_sample_lost(reader,
status)` mit dem auslösenden Reader.

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: Conditions und WaitSet

### §2.1 Condition: get_trigger_value

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.7, S. 31–32 (`dds-1.2.pdf`), Tabelle
Condition — "get_trigger_value: boolean"

**Repo:** `crates/cpp/include/dds/core/cond/TCondition.hpp::TCondition::trigger_value`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `trigger_value()` der GuardCondition, ReadCondition und
StatusCondition; der Test prüft den Anfangswert FALSE der GuardCondition und das Setzen.

### §2.1 WaitSet: Erzeugen ohne Factory

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`) — "WaitSet
has no factory. It is created as an object directly by the natural means in each
language binding (e.g., using “new” in C++ or Java)."

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::TWaitSet`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `WaitSet()` erzeugt ein WaitSet ohne Factory (Standardkonstruktor).

### §2.1 WaitSet: attach_condition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "attach_condition (a_condition: Condition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::attach_condition`,
`crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::operator+=`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `attach_condition(cond)` und `ws += cond` hängen eine Bedingung an.

### §2.1 WaitSet: detach_condition

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "detach_condition (a_condition: Condition): ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::detach_condition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `detach_condition(cond)` gibt wahr für eine angehängte, falsch für
eine nicht angehängte Bedingung.

### §2.1 WaitSet: wait

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "wait (inout: active_conditions: Condition [], timeout: Duration_t):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`,
`crates/cpp/include/dds/core/cond/detail/WaitSetDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait(timeout)`, `wait()` (unbegrenzt) und die Formen mit
`ConditionSeq&` blockieren bis eine Bedingung gilt; ohne gültige Bedingung endet
`wait(timeout)` mit `TimeoutError`; `dispatch(timeout)` ruft den Handler jeder gültigen
Bedingung.

### §2.1 WaitSet: Ergebnis von wait

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "inout: active_conditions: Condition []"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `wait` gibt die Bedingungen zurück, die gelten (`ConditionSeq`);
der Test prüft die Zahl.

### §2.1 WaitSet: get_conditions

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "get_conditions (inout: attached_conditions: Condition []):
ReturnCode_t"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::conditions`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `conditions()` gibt die angehängten Bedingungen zurück.

### §2.1 GuardCondition: Erzeugen ohne Factory, Anfangswert FALSE

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.8, S. 32 (`dds-1.2.pdf`) —
"GuardCondition has no factory. It is created as an object directly by the
natural means in each language binding (e.g., using “new” in C++ or Java). When
first created the trigger_value is set to FALSE."

**Repo:**
`crates/cpp/include/dds/core/cond/TGuardCondition.hpp::TGuardCondition::TGuardCondition`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `GuardCondition()`; `trigger_value()` ist anfangs falsch.

### §2.1 GuardCondition: set_trigger_value

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.8, S. 32 (`dds-1.2.pdf`), Tabelle
GuardCondition — "set_trigger_value (value: boolean): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/core/cond/TGuardCondition.hpp::TGuardCondition::trigger_value`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`

**Status:** `done` — `trigger_value(bool)` setzt den Wert; ein angehängtes WaitSet sieht
ihn.

### §2.1 StatusCondition: set_enabled_statuses und get_enabled_statuses

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.9, S. 32 (`dds-1.2.pdf`), Tabelle
StatusCondition — "set_enabled_statuses (mask: StatusKind []): ReturnCode_t;
get_enabled_statuses: StatusKind []"

**Repo:**
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition::enabled_statuses`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `enabled_statuses(mask)` setzt, `enabled_statuses()` liest die
Maske.

### §2.1 StatusCondition: get_entity

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.1.9, S. 32 (`dds-1.2.pdf`), Tabelle
StatusCondition — "get_entity: Entity"

**Repo:**
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp::TStatusCondition::entity`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `entity()` gibt die Entität der Bedingung zurück (commit
`eb8bfdd601e`).

### §2.1 ReadCondition: get_datareader

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.8, S. 106 (`dds-1.2.pdf`), Tabelle
ReadCondition — "get_datareader: DataReader"

**Repo:**
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp::TReadCondition::data_reader`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `data_reader()` gibt den Reader der Bedingung zurück.

### §2.1 ReadCondition: get_sample_state_mask, get_view_state_mask, get_instance_state_mask

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.8, S. 106 (`dds-1.2.pdf`), Tabelle
ReadCondition — "get_sample_state_mask: SampleStateKind []; get_view_state_mask:
ViewStateKind []; get_instance_state_mask: InstanceStateKind []"

**Repo:**
`crates/cpp/include/dds/sub/cond/TReadCondition.hpp::TReadCondition::state_filter`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `state_filter()` gibt die `DataState` mit Sample-, View- und
Instance-State zurück.

### §2.1 QueryCondition: get_query_expression

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.9, S. 107 (`dds-1.2.pdf`), Tabelle
QueryCondition — "get_query_expression: string"

**Repo:**
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::expression`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — `expression()` liest, `expression(text)` ersetzt den Ausdruck.

### §2.1 QueryCondition: get_query_parameters und set_query_parameters

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.2.5.9, S. 107 (`dds-1.2.pdf`), Tabelle
QueryCondition — "get_query_parameters (out: query_parameters: string []):
ReturnCode_t; set_query_parameters (query_parameters: string []): ReturnCode_t"

**Repo:**
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::parameters`,
`crates/cpp/include/dds/sub/cond/ReadCondition.hpp::QueryCondition::add_parameter`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/zerodds-c-api/tests/query_condition_update.rs::a_query_condition_takes_a_new_expression_and_new_parameters`

**Status:** `done` — `parameters_length()`, `begin()`, `end()` lesen, `parameters(begin,
end)` ersetzt, `add_parameter` ergänzt (commit `eb8bfdd601e`).

---

## §2.1 Programmierschnittstellen von [DDS] 1.2: eingebaute Topics

### §2.1 Eingebaute Topics: DCPSParticipant

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 148 (`dds-1.2.pdf`) —
"DCPSParticipant: key BuiltinTopicKey_t; user_data UserDataQosPolicy"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::ParticipantBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `ParticipantBuiltinTopicData` trägt `key()` und `user_data()` (auch als `ParticipantTopicData`); `get_discovered_participant_data(handle)` füllt sie, und der Reader `DataReader<ParticipantBuiltinTopicData>` des Built-in-Subscriber liefert die Samples des Topic DCPSParticipant mit `read()`/`take()` (`test_builtin_topic_readers`: ein Participant erscheint, `take` entfernt ihn).

### §2.1 Eingebaute Topics: DCPSTopic

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 148 (`dds-1.2.pdf`) — "DCPSTopic:
key, name, type_name, durability, durability_service, deadline, latency_budget,
liveliness, reliability, transport_priority, lifespan, destination_order,
history, resource_limits, ownership, topic_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::TopicBuiltinTopicData`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_discovered_topic_data_unknown_handle_throws`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `TopicBuiltinTopicData` trägt jedes Mitglied von DCPSTopic als Policy-Klasse (commit `e0b3af9f2c7`); `get_discovered_topic_data(handle)` liefert die Daten, und der Reader `DataReader<TopicBuiltinTopicData>` liefert die Samples des Topic DCPSTopic (`test_builtin_topic_readers`: das Topic „BuiltinReaders“ erscheint mit seinem Namen).

### §2.1 Eingebaute Topics: DCPSPublication

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 149 (`dds-1.2.pdf`) —
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

**Status:** `done` — `PublicationBuiltinTopicData` trägt die Mitglieder von DCPSPublication (Schlüssel, `participant_key()`, Topic- und Typname, Durability, Deadline, LatencyBudget, Liveliness, Reliability, UserData, Ownership, DestinationOrder, Presentation, Partition, TopicData, GroupData, DurabilityService, Lifespan, OwnershipStrength); `matched_publication_data(reader, handle)` füllt sie, und der Reader `DataReader<PublicationBuiltinTopicData>` liefert die Samples des Topic DCPSPublication (`test_builtin_topic_readers`: die Publication des Writers der anderen Participant erscheint mit ihrem Topic-Namen). Die Werte der übrigen Mitglieder liest kein Test (siehe das folgende Item).

### §2.1 Eingebaute Topics: übrige Felder von DCPSPublication

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 149 (`dds-1.2.pdf`) —
"DCPSPublication: key, participant_key, type_name, durability,
durability_service, latency_budget, liveliness, reliability, lifespan,
ownership, ownership_strength, destination_order, presentation, partition,
topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::PublicationBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp::TPublicationBuiltinTopicData`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_values_arrive`

**Status:** `done` — Alle Mitglieder kommen mit dem Wert an, mit dem der Writer angelegt wurde: `test_builtin_topic_values_arrive` liest die Publication aus dem Built-in-Reader der anderen Participant und prüft Typname, DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS, RELIABILITY, DESTINATION_ORDER, USER_DATA, OWNERSHIP, OWNERSHIP_STRENGTH, LIFESPAN, PARTITION, GROUP_DATA, TOPIC_DATA, PRESENTATION, DURABILITY_SERVICE, Schlüssel und Participant-Schlüssel. Der Test fand, dass TOPIC_DATA des Topic nicht ankam; ein Writer und ein Reader melden jetzt das TOPIC_DATA ihres Topic.

### §2.1 Eingebaute Topics: DCPSSubscription

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, topic_name, type_name, durability,
deadline, latency_budget, liveliness, reliability, ownership, destination_order,
user_data, time_based_filter, presentation, partition, topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::SubscriptionBuiltinTopicData`,
`crates/cpp/include/dds/pub/discovery.hpp::dds::pub::matched_subscription_data`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`,
`crates/zerodds-c-api/tests/builtin_reader.rs::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`

**Status:** `done` — `SubscriptionBuiltinTopicData` trägt die Mitglieder von DCPSSubscription einschließlich `time_based_filter()`; `matched_subscription_data(writer, handle)` füllt sie, und der Reader `DataReader<SubscriptionBuiltinTopicData>` liefert die Samples des Topic DCPSSubscription (`test_builtin_topic_readers`: das Subscription des Readers der anderen Participant erscheint mit seinem Topic-Namen, gesehen von der Seite des Writers).

### §2.1 Eingebaute Topics: übrige Felder von DCPSSubscription

**Spec:** §2.1, S. 11; [DDS] 1.2 §7.1.5, S. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, durability, deadline, latency_budget,
liveliness, reliability, ownership, destination_order, user_data, presentation,
partition, topic_data, group_data"

**Repo:** `crates/cpp/include/dds/topic/BuiltinTopic.hpp::SubscriptionBuiltinTopicData`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp::TSubscriptionBuiltinTopicData`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_values_arrive`

**Status:** `done` — Die Mitglieder kommen mit dem Wert an, mit dem der Reader angelegt wurde: `test_builtin_topic_values_arrive` prüft DURABILITY, DEADLINE, LATENCY_BUDGET, RELIABILITY, OWNERSHIP, USER_DATA, PARTITION, TIME_BASED_FILTER und TOPIC_DATA der Subscription aus dem Built-in-Reader der Writer-Seite.

---

## §2.2 Programming Interfaces

### §2.2 Namen und Orte der Header unter `dds` normativ

**Spec:** §2.2, S. 11 — "The file names and relative locations of all C++
headers within the “dds” directory are normative. Those headers within “detail”
subdirectories are excepted; they are not normative."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Von den 196 Dateien der OMG außerhalb von `detail/` (153 öffentliche) liegen alle an demselben Ort unter `crates/cpp/include/dds/`. Verglichen mit einem Skript über die Dateilisten (Cache `dds-psm-cxx-1.0-sources/hpp/dds`).

### §2.2 Öffentliche Namen in `::dds::` normativ

**Spec:** §2.2, S. 12 — "All public symbol names within the ::dds:: namespace
and its contained namespaces, including those names introduced into those
namespaces by means of typedef declarations, are normative. Those names within
“detail” namespaces are excepted; they are not normative."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_reference_type_macros`

**Status:** `done` — Alle Bezeichner der öffentlichen Header der OMG (Klassen, Typedefs, Funktionen, Makros) sind vorhanden, auch `OMG_DDS_REF_TYPE_BASE`, `OMG_DDS_REF_TYPE_BASE_T`, `OMG_DDS_REF_TYPE_DELEGATE_T` und `DDS_PTR_CTOR_REF_TYPE` in `dds/core/refmacros.hpp` (Test `test_reference_type_macros`). Wo die OMG-Header einen Namen falsch schreiben (`offered_deadlined_missed_status`, `offered_incompatible_qos_staus`), gibt es beide Schreibweisen.

### §2.2 Verteilung der Namen auf die Header normativ

**Spec:** §2.2, S. 12 — "The distribution of the normative symbol names among
the normative headers is itself normative, such that a source file that includes
the header in which a given name is declared will continue to compile when that
header is replaced with the corresponding header from a different DDS
implementation."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own`

**Status:** `done` — Für jeden Namen der öffentlichen OMG-Header (201 Namen, außerhalb von `detail/`)
übersetzt eine eigene Einheit, die nur denselben Header einbindet und den Namen in
demselben Namensraum anspricht. Alle 201 sind erreichbar. Dazu mussten
`MultiTopic` über `TMultiTopic.hpp` und `dds::code` über `DynamicData.hpp` erreichbar
werden; die zweite Schreibweise entspricht einem Tippfehler der OMG-Header und ist als
Alias vorhanden.

### §2.2 Übrige Dateien sind Referenzimplementierung und Beispiele

**Spec:** §2.2, S. 12 — "The remainder of the files, declarations, and
definitions contained within this specification’s C++ programming interfaces
constitute a reference implementation and a set of examples. They are not
normative."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage über den Status der Dateien; ZeroDDS liefert
die Header unter `dds/`, die Header unter `zerodds/` sind eigene API.

### §2.2 Keine Erweiterungen des Herstellers in normativen Namensräumen

**Spec:** §2.2, S. 12 — "Conforming implementations shall not define
implementation-specific extension programming interfaces within normative
namespaces. They may, however, specialize normative templates defined by this
specification."

**Repo:** `crates/cpp/include/dds/core/xtypes/EnumTypes.hpp`,
`crates/cpp/include/dds/core/xtypes/CollectionTypes.hpp`,
`crates/cpp/include/dds/sub/find.hpp`,
`crates/cpp/tests/vendor_names.txt`

**Tests:** `crates/cpp/tests/public_headers.rs::no_vendor_name_in_the_normative_namespaces`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_builtin_topic_readers`

**Status:** `done` — `tools/psm-omg-names.py` listet die Namen auf Namespace-Ebene aus unseren
öffentlichen Headern außerhalb von `detail`. Jeder Name kommt als Bezeichner in den
OMG-Headern vor (`omg_words.txt`) oder steht mit Begründung in `vendor_names.txt`
(13 Einträge; ein Eintrag, den die Header nicht mehr brauchen, lässt den Test
scheitern). Entfernt oder verschoben in dieser Runde: `DomainParticipantFactoryQos` (ungenutzt,
gelöscht), `BuiltinSubscriber` (`dds::sub::builtin_subscriber(dp)` gibt jetzt ein
`dds::sub::Subscriber` zurück, `dds::sub::find` findet darauf die vier Builtin-Reader),
`TTopic`, `TMultiTopic`, `TContentFilteredTopic`, `TTopicDescription`, `TDataReader`
(jetzt in `detail`). Die Einträge von `vendor_names.txt`: `dds::domain::DomainId`
(die Beispiele in §7.13 schreiben `DomainId id = 0;`) und zwölf Klassen in `dds::core::xtypes`
(`AliasType`, `ArrayType`, `TArrayType`, `BitField`, `BitFlag`, `BitMaskType`, `BitSetType`,
`EnumerationType`, `Enumerator`, `OptionalAnnotation`, `TOptionalAnnotation`,
`StructForwardDeclaration`); Grundlage ist §7.11 („all classes necessary for describing
dynamic types“) mit den TypeKinds aus XTypes 1.3. Das ist eine Auslegung von §7.11, die
OMG-Header haben für diese Arten keine Klasse. Nicht erfasst: Namen, die ein Makro
deklariert.

---

## §3 Normative References

### §3 [C99] ISO/IEC 9899:1999

**Spec:** §3, S. 12 — "[C99] C Programming Language (ISO/IEC 9899:1999)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; nach §7.4.2 für die Ganzzahltypen von
`stdint.h`. Die Header binden `<stdint.h>` ein, wo sie die Typen global nennen (commit
`def4ff43f03`); siehe die Items zu §7.4.2.

### §3 [C++] ISO/IEC 14882:2003

**Spec:** §3, S. 12 — "[C++] C++ Programming Language (ISO/IEC 14882:2003)"

**Repo:** `crates/cpp/include/zerodds/psm/cxx.hpp`, `crates/cpp/include/dds/`

**Tests:**
`crates/cpp/tests/public_headers.rs::every_public_header_compiles_on_its_own_as_cxx03`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_as_cxx03`

**Status:** `done` — Alle 144 öffentlichen Header unter `crates/cpp/include/dds` übersetzen
einzeln mit `-std=c++03 -pedantic`. Geprüft mit g++ 12.2 (`tools/psm-cxx03-check.py --cxx g++` auf dem Linux-Testhost) und
mit clang, bei dem die C++11-Erweiterungen Fehler sind (Test
`every_public_header_compiles_on_its_own_as_cxx03`, grün auf dem Linux-Testhost). Das Smoke-Programm
`smoke_dds_psm.cpp` wird mit g++ als C++03 übersetzt und ausgeführt (Test
`dds_psm_cxx_smoke_as_cxx03`, grün auf dem Linux-Testhost); als C++17 läuft es weiter wie bisher. Die Header
benutzen `zerodds/psm/cxx.hpp`: mit einem Übersetzer ab C++11 sind das die Namen der
Standardbibliothek (`shared_ptr`, `mutex`, `function`, `array`), mit C++03 eigene Klassen
(Referenzzählung, `mutex` über POSIX-Threads oder Windows, `function` bis zwei Argumente,
`array`, Typmerkmale). C++11-Zusätze der Spec (§7.12: Verschieben, Initialisierungslisten,
`enum class`, `noexcept`) stehen hinter `#if ZERODDS_CXX11`. Die Tests von Threads und von
Verschiebeoperationen laufen nur mit C++11. Nicht C++03: `zerodds/dds.hpp` und
`zerodds/psm/convert.hpp` (der Codec `runtime/cpp/include/zerodds/wire.hpp` und der erzeugte
Code von `idlc` verlangen C++17); die Header unter `dds/` brauchen sie nicht (nur
`zerodds::wire::Encoding`, in C++03 ein eigener Aufzählungstyp in `zerodds/psm/encoding.hpp`).
Nicht gelaufen: das CMake-Ziel `zerodds_psm_standalone_headers` mit `CXX_STANDARD 98`.

### §3 [DDS] Version 1.2

**Spec:** §3, S. 12 — "[DDS] Data Distribution Service for Real-Time Systems
Specification, version 1.2 (OMG document formal/2007-01-01)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; die Operationen von DCPS stehen unter §2.1
nach dieser Version. ZeroDDS implementiert DCPS nach DDS 1.4 (`dds-dcps-1.4.md`).

### §3 [DDS-XTypes] 1.0 Beta 1

**Spec:** §3, S. 12 — "[DDS-XTypes] Extensible and Dynamic Topic Types, version
1.0 Beta 1 (OMG document ptc/2010-05-12)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf eine Fassung, die nicht im Cache liegt;
ZeroDDS implementiert XTypes 1.3 (`dds-xtypes-1.3.md`).

### §3 [DDS-CCM] 1.0 Beta 1

**Spec:** §3, S. 12 — "[DDS-CCM] DDS for Lightweight CCM, version 1.0 Beta 1
(OMG document ptc/2009-02-02)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf eine Fassung, die nicht im Cache liegt;
die XML-QoS-Profile liest `QosProvider` nach DDS-XML 1.0 (`dds-xml-1.0.md`).

---

## §4 Terms and Definitions

### §4 Data Centric Publish-Subscribe (DCPS)

**Spec:** §4, S. 12 — "The mandatory portion of the DDS specification used to
provide the functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §4 Data Distribution Service for Real-Time Systems (DDS)

**Spec:** §4, S. 12 — "An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness and
reliability. It is independent of implementation languages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §4 Data Local Reconstruction Layer

**Spec:** §4, S. 12 — "The optional portion of the DDS specification used to
provide the functionality required for an application for direct access to data
exchanged at the DCPS layer. This later builds upon the DCPS layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §4 Platform-Independent Model (PIM)

**Spec:** §4, S. 13 — "An abstract definition of a facility, often expressed
with the aid of formal or semi-formal modeling languages such as OMG UML that
does not depend on any particular implementation technology."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §4 Platform-Specific Model (PSM)

**Spec:** §4, S. 13 — "A concrete definition of a facility, typically based on a
corresponding PIM, in which all implementation-specific dependencies have been
resolved."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

---

## §5 Symbols

### §5 Symbol `<:`

**Spec:** §5, S. 13 — "The symbol “<:” is the commonly used symbol to denote
subtyping. Given two programming language type T and Q, we can say that Q <: T
if any occurrence of T can be replaced by Q."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §5 Symbol `Foo<+T>`

**Spec:** §5, S. 13 — "When Foo is a class parameterized on the type T, we use
the notation Foo<+T> to indicate that Foo is covariant in T. This means that
given Q <: T then Foo<Q> <: Foo<T> When no annotation is provided then the class
is supposed to be invariant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §5 Symbol `Foo<-T>`

**Spec:** §5, S. 13 — "When Foo is a class parameterized on the type T, we use
the notation Foo<- T> to indicate that Foo is contra-variant in T. This means
that given Q <: T then Foo<T> <: Foo<Q> When no annotation is provided then the
class is supposed to be invariant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

### §5 Symbol `Foo<T>`

**Spec:** §5, S. 13 — "When foo is non-variant in T."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

---

## §6 Additional Information

### §6.1 Acknowledgments

**Spec:** §6.1, S. 13 — "The following companies submitted this specification: •
PrismTech Corporation, Ltd. • Real-Time Innovations, Inc. (RTI)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung oder Notation der Spec; keine
Anforderung.

---

## §7.1 Overview

### §7.1 Anlass: eine native C++-API statt der aus IDL abgeleiteten

**Spec:** §7.1, S. 15 — "The “ISO/IEC C++ Language DDS PSM” (DDS-PSM-Cxx) was
motivated by mainly two reasons. First the IDL-derived C++ API for DDS does not
integrate well with the C++ language [...] As a result this specification takes
a completely fresh look at how a native C++ PSM can be derived from the DDS
PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Spec; keine Anforderung.

### §7.1 Keine Merkmale von C++11 nötig, ihre Nutzung möglich

**Spec:** §7.1, S. 15 — "This specification does not require C++11 features for
its implementation, yet it is designed to enable the use of C++11 features, such
as the auto keyword, range-based for loops, etc."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage zum Entwurf: die Header brauchen C++11 nicht (sie übersetzen als C++03, siehe §3 [C++]); mit einem C++11-Übersetzer lassen sich `auto`, bereichsbasierte `for`-Schleifen und Verschieben benutzen (siehe §7.12).

---

## §7.2 Specification Organization

### §7.2 Namensräume nach den Modulen des PIM von DDS 1.2

**Spec:** §7.2, S. 15 — "The DDS-PSM-Cxx API is organized around namespaces that
match the different modules defined by the DDS v1.2 PIM (see Figure 7.1)."

**Repo:** `crates/cpp/include/dds/domain/`, `crates/cpp/include/dds/topic/`,
`crates/cpp/include/dds/pub/`, `crates/cpp/include/dds/sub/`,
`crates/cpp/include/dds/core/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `dds::core`, `dds::domain`, `dds::topic`, `dds::pub`, `dds::sub`
mit den Verzeichnissen und Namensräumen der OMG-Header.

### §7.2 Namensräume unterhalb der Module nach Figure 7.1

**Spec:** §7.2, S. 15, Figure 7.1 — "Figure 7.1 - Standard Packages
Organization" — core: xtypes, policy, detail, cond, status; domain: detail, qos;
Topic: qos, detail; pub: qos, detail; sub: qos, cond, status, detail

**Repo:** `crates/cpp/include/dds/core/xtypes/`, `crates/cpp/include/dds/core/policy/`,
`crates/cpp/include/dds/core/cond/`, `crates/cpp/include/dds/core/status/`,
`crates/cpp/include/dds/domain/qos/`, `crates/cpp/include/dds/topic/qos/`,
`crates/cpp/include/dds/pub/qos/`, `crates/cpp/include/dds/sub/qos/`,
`crates/cpp/include/dds/sub/cond/`, `crates/cpp/include/dds/sub/status/`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_readcondition_lifecycle`

**Status:** `done` — Die Namensräume `core::{xtypes, policy, cond, status}`,
`domain::qos`, `topic::qos`, `pub::qos`, `sub::{qos, cond, status}` und jeweils `detail`
sind vorhanden und werden von den Tests benutzt.

### §7.2 Namensraum `dds::sub::status`

**Spec:** §7.2, S. 15, Figure 7.1; §7.13.2, S. 32 — "sub: qos, cond, status,
detail"; ".data(dds::sub::status::DataState::new_data())"

**Repo:** `crates/cpp/include/dds/sub/status/DataState.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::sub::status::DataState` mit `SampleState`, `ViewState`,
`InstanceState` und `DataState::new_data()`, `::any()`.

### §7.2 `dds::core` enthält Time, Duration, Policies, Referenz- und Werttypen

**Spec:** §7.2, S. 15 — "The dds::core - as implied by its name - provides core
abstractions that are used throughout the API, such as the Time and Duration,
the Policies, and the definition of reference and value types."

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`,
`crates/cpp/include/dds/core/policy/CorePolicy.hpp`,
`crates/cpp/include/dds/core/Reference.hpp`, `crates/cpp/include/dds/core/Value.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core` enthält `Time`, `Duration`, die Policies,
`Reference<D>`, `WeakReference<T>` und `Value<D>`.

### §7.2 Typkonstruktoren mit Delegate als Parameter

**Spec:** §7.2, S. 15 — "The specification defines type constructors, i.e.,
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

**Status:** `done` — Die Klassen sind Templates über einen Delegate
(`TDomainParticipant<DELEGATE>`, `TPublisher<DELEGATE>`, …, commits `ee3c3473721`,
`0c023caded7`); die Delegates liegen in `detail/`.

### §7.2 Unterpakete `detail` sind keine Konformitätspunkte

**Spec:** §7.2, S. 15 — "The “detail” sub-packages visible in Figure 7.1, are
intended to store the “link” between the standard API and the vendor
implementation. The content of the detail sub-package is provided as a guideline
and does not constitute a point of compliance."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage der Spec; die Delegates von ZeroDDS liegen in
den `detail`-Unterpaketen.

### §7.2 Pakete mit geringen Abhängigkeiten

**Spec:** §7.2, S. 15 — "The DDS-PSM-Cxx organizes DDS classes as a set of
packages that maximize the coherence and minimize the dependencies across
packages. This organization minimizes API dependencies and reduce the include
files required by publish, or subscribe, only applications speeding up
compilation times."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage zum Entwurf; jeder öffentliche Header
übersetzt allein (CMake-Ziel `zerodds_psm_standalone_headers`).

### §7.2 Typkonstruktor `TInstanceHandle` in `dds/core/TInstanceHandle.hpp`

**Spec:** §7.2, S. 15–16 — "For instance if we take as an example the type
constructor TInstanceHandle, specified in the file dds/core/TInstanceHandle.hpp
as: namespace dds { namespace core { template <typename DELEGATE> class
TInstanceHandle ; } }"

**Repo:** `crates/cpp/include/dds/core/TInstanceHandle.hpp::TInstanceHandle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `dds::core::TInstanceHandle<DELEGATE>` in
`dds/core/TInstanceHandle.hpp`; `nil()`, `is_nil()`, `value()`.

### §7.2 Instanziierung durch den Hersteller in `detail`

**Spec:** §7.2, S. 16 — "Then its instantiation is to be defined by the
implementor of the API within the dds::core::delegate namespace as something
like: namespace dds { namespace core { namespace detail { typedef
dds::core::TInstanceHandle<foo::core::InstanceHandleDelegate> InstanceHandle; }
}"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel der Spec; ZeroDDS instanziiert die
Typkonstruktoren in `detail`-Namensräumen (z. B.
`dds/core/detail/InstanceHandle`-Delegate in `dds/core/InstanceHandle.hpp`).

### §7.2 `dds::core::InstanceHandle` in `dds/core/InstanceHandle.hpp`

**Spec:** §7.2, S. 16 — "This instantiation of the type constructor
TInstanceHandle is then used by the standard API in the
dds/core/InstanceHandle.hpp file to define the standard instance handle as:
namespace dds { namespace core { typedef detail::InstanceHandle InstanceHandle;
} }"

**Repo:** `crates/cpp/include/dds/core/InstanceHandle.hpp::InstanceHandle`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::InstanceHandle` ist die Instanz von `TInstanceHandle`
in `dds/core/InstanceHandle.hpp`.

### §7.2 Kein Hersteller ändert die öffentliche API

**Spec:** §7.2, S. 16 — "Under no circumstances a vendor shall change the public
API defined by this specification."

**Repo:** `crates/cpp/include/dds/.`

**Tests:** `crates/cpp/tests/public_headers.rs::every_name_of_the_omg_headers_is_reachable_through_its_header`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_signatures_follow_the_normative_headers`,
`crates/cpp/tests/public_headers.rs::no_vendor_name_in_the_normative_namespaces`

**Status:** `done` — Namen und Verteilung entsprechen den OMG-Headern (siehe §2.2), die Member-Signaturen
wurden mit `tools/psm-signature-diff.py` verglichen und angeglichen (siehe §2); die
zusätzlichen Namen stehen im Item „§2.2 Keine Erweiterungen des Herstellers in
normativen Namensräumen“. Nicht verglichen: Ausnahmespezifikationen, Default-Argumente.

### §7.2 Typkonstruktoren delegieren nur, Erweiterungen im Delegate

**Spec:** §7.2, S. 16 — "The only action performed by type constructor is to
delegate their implementation to the DELEGATE template parameter. It is the
DELEGATE type that provides the actual implementation and that encapsulate
vendor extensions."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — Die Typkonstruktoren rufen den Delegate; die Erweiterungen liegen
im Delegate und sind über `operator->` erreichbar (`dp->find_topic(…)`,
`dw->wait_for_matched(…)`).

### §7.2 Standardweg zu Erweiterungen des Herstellers

**Spec:** §7.2, S. 16 — "The DDS-PSM-Cxx API provides a standard way of
accessing vendor specific extensions."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::operator->`,
`crates/cpp/include/dds/core/Value.hpp::Value::operator->`,
`crates/cpp/include/dds/core/Reference.hpp::Reference::delegate`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `operator->` und `delegate()` geben den Delegate (§7.6, S. 25);
`dp->delete_contained_entities()`, `dw->wait_for_matched(…)`.

### §7.2 Einbinden der ganzen API über `dds/dds.hpp`

**Spec:** §7.2, S. 16 — "1. The entire DDS API can be included at once: •
#include <dds/dds.hpp>"

**Repo:** `crates/cpp/include/dds/dds.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `#include <dds/dds.hpp>` bindet die ganze API; der Test bindet nur
diesen Header (und die Header der Zusatzfunktionen) ein.

### §7.2 Einbinden eines Moduls über `dds/module/ddsmodule.hpp`

**Spec:** §7.2, S. 16 — "2. Individual DDS modules can be included. These
headers have the form dds/module/ddsmodule.hpp. For example: • #include
<dds/pub/ddspub.hpp>"

**Repo:** `crates/cpp/include/dds/core/ddscore.hpp`,
`crates/cpp/include/dds/domain/ddsdomain.hpp`,
`crates/cpp/include/dds/topic/ddstopic.hpp`, `crates/cpp/include/dds/pub/ddspub.hpp`,
`crates/cpp/include/dds/sub/ddssub.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Die Modul-Header `dds/core/ddscore.hpp`,
`dds/domain/ddsdomain.hpp`, `dds/topic/ddstopic.hpp`, `dds/pub/ddspub.hpp`,
`dds/sub/ddssub.hpp` sind vorhanden und übersetzen allein.

### §7.2 Einbinden einer Klasse über `dds/module/ClassName.hpp`

**Spec:** §7.2, S. 16 — "3. Individual types can be included. These headers have
the form dds/module/ClassName.hpp. For example: • #include
<dds/pub/DataWriter.hpp>"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp`,
`crates/cpp/include/dds/sub/DataReader.hpp`,
`crates/cpp/include/dds/domain/DomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — Jede Klasse hat ihren Header (`dds/pub/DataWriter.hpp`, …), und
jeder öffentliche Header übersetzt allein (CMake-Ziel `zerodds_psm_standalone_headers`).

---

## §7.3 Concurrency, Reentrancy and Exception Safety

### §7.3 Operationen von DataReader und DataWriter reentrant

**Spec:** §7.3, S. 16 — "All DataReader and DataWriter operations shall be
reentrant."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_operations_are_called_from_several_threads`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_under_thread_sanitizer`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::the_threaded_smoke_tests_pass_without_the_sanitizer`

**Status:** `done` — Vier Schreiber-Threads auf einem DataWriter, ein Thread, der Status und QoS liest, und ein Leser, der nimmt: alle 200 Samples kommen an, keine Operation wirft. Der Test läuft auch mit `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 Berichte, alle im Modul `libzerodds` (Rust, vom Prüfer nicht instrumentiert, seine Sperren und Kanäle sind ihm unsichtbar), keiner im C++-Code; die Prüfung schlägt fehl, sobald ein Bericht außerhalb der Bibliothek liegt. Die Berichte in der Bibliothek sind damit weder bestätigt noch widerlegt.

### §7.3 Lesen und Nehmen über Loans ausnahmesicher

**Spec:** §7.3, S. 16 — "Loand-based read/take operation shall be exception
safe."

**Repo:** `crates/cpp/include/dds/sub/detail/SampleDetail.hpp::LoanGuard`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_returns_the_loan_when_decode_throws`

**Status:** `done` — `LoanGuard` gibt den Loan zurück, auch wenn die Dekodierung wirft
(commit `409a44a86e9`); der Reader bleibt benutzbar.

### §7.3 Konstruktoren und Kopierzuweisung von Werttypen ausnahmesicher

**Spec:** §7.3, S. 17 — "Constructors and copy-assignment operators of normative
classes that inherit from Value<D> and the Value<D> template itself shall
preferably be exception safe. Deviation from this norm should be carefully noted
on vendor documentation."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`

**Tests:** —

**Status:** `n/a (informative)` — Eine Empfehlung („preferably“). `Value<D>` kopiert den
Delegate mit seiner Kopierzuweisung; die Policy-Klassen halten `std::vector`,
`std::string` und Zahlen. Kein Test lässt eine Kopie scheitern, und die Dokumentation
von `crates/cpp` nennt keine Abweichung.

### §7.3 Operationen von Topic, Publisher, Subscriber und DomainParticipant reentrant

**Spec:** §7.3, S. 17 — "All Topic (and other TopicDescription extension
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

**Status:** `done` — Der Test ruft `qos()` von Publisher, Subscriber und Topic, `current_time()` und `contains_entity()` des Participant aus einem Thread auf, während Writer schreiben; `test_entities_are_created_from_several_threads` legt aus vier Threads je zehn Topics, Publisher, Subscriber, Writer und Reader an einem Participant an und schließt sie. Die Verzeichnisse der Delegates (`ParticipantRegistry`, `EntityTracker`, `ChildTracker`, `RetainedEntities`) sind durch Mutexe geschützt. `close()` nebenläufig aufzurufen schließt die Spec aus. Der Test läuft auch mit `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 Berichte, alle im Modul `libzerodds` (Rust, vom Prüfer nicht instrumentiert, seine Sperren und Kanäle sind ihm unsichtbar), keiner im C++-Code; die Prüfung schlägt fehl, sobald ein Bericht außerhalb der Bibliothek liegt. Die Berichte in der Bibliothek sind damit weder bestätigt noch widerlegt.

### §7.3 Operationen der DomainParticipantFactory reentrant

**Spec:** §7.3, S. 17 — "All DomainParticipantFactory operations shall be
reentrant with the exception that DomainParticipantFactory.close may not be
called on a given object concurrently with any other call of any method on that
object or on any contained object."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben keine
DomainParticipantFactory (siehe „DomainParticipantFactory: get_instance“); die Erzeugung
eines Participant ist `DomainParticipant(domain_id, …)`.

### §7.3 Operationen von WaitSet und Condition reentrant

**Spec:** §7.3, S. 17 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception that
their close() operations may not be invoked concurrently with any other method
on the same object."

**Repo:** `crates/cpp/include/dds/core/cond/detail/WaitSetDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_guardcondition_and_waitset`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_conditions_are_used_from_several_threads`,
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke_under_thread_sanitizer`

**Status:** `done` — `test_conditions_are_used_from_several_threads`: vier Threads setzen und lesen GuardConditions, ein Thread wartet in einem WaitSet, ein Thread hängt eine Condition an und ab und liest `conditions()`; 25 Runden, keine Operation wirft. Der Test fand einen Fehler: die Liste der angehängten Conditions in `WaitSetDelegate` und der Handler in `GuardConditionDelegate` hatten keine Sperre, das Programm stürzte unter gleichzeitigem Anhängen, Abhängen und Warten ab (SIGSEGV); beide sind jetzt durch einen Mutex geschützt. Der Test läuft auch mit `-fsanitize=thread` (`dds_psm_cxx_smoke_under_thread_sanitizer`): 249 Berichte, alle im Modul `libzerodds` (Rust, vom Prüfer nicht instrumentiert, seine Sperren und Kanäle sind ihm unsichtbar), keiner im C++-Code; die Prüfung schlägt fehl, sobald ein Bericht außerhalb der Bibliothek liegt. Die Berichte in der Bibliothek sind damit weder bestätigt noch widerlegt.

### §7.3 Im Listener nur die auslösende Entität aufrufen

**Spec:** §7.3, S. 17 — "Code within a DDS listener callback may not safely call
any method on any DDS Entity but the one on which the status change occurred."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung für den Anwendungscode im Rückruf; sie
verlangt nichts von der Implementierung.

### §7.3 Methoden von Werttypen dürfen nicht reentrant sein

**Spec:** §7.3, S. 17 — "Any method of any value type may be non-reentrant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; keine Anforderung.

### §7.3 Stärkere Zusagen erlaubt

**Spec:** §7.3, S. 17 — "A Service implementation may choose to provide
unspecified stronger guarantees than the rules above."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; keine Anforderung.

---

## §7.4 General Rules for Mapping the DDS PIM to the DDS-PSM-Cxx

### §7.4 Allgemeine Regeln für einen Teil der Klassen

**Spec:** §7.4, S. 17 — "This specification defines some general rules to map
DDS PIM classes to DDS-PSM-Cxx classes. These rules are applicable to a subset
of classes, luckily the most numerous, while special mapping is required for
some of the DDS entities as described below."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu den Regeln von §7.4.1 bis §7.4.6; jede
Regel hat ein eigenes Item.

### §7.4.1 Jede Klasse des PIM wird eine Klasse in C++

**Spec:** §7.4.1, S. 17 — "As a general rule all classes included in the DDS PIM
have to be mapped into a C++ class. The specific nature of this class depends on
whether the DDS PIM element has reference or value semantics."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`,
`crates/cpp/include/dds/core/Value.hpp`, `crates/cpp/include/dds/core/TEntity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Entitäten, Bedingungen und `Any*`-Klassen sind Referenztypen
(`Reference<DELEGATE>`, commits `ee3c3473721`, `0c023caded7`); Status, QoS, Policies,
Zeiten und Samples sind Werttypen (`Value<D>`). Alle sind Klassen.

### §7.4.1 Keine Klasse des PIM wird ein `struct`

**Spec:** §7.4.1, S. 17 — "Note – An implication of this mapping is that no DDS
PIM class ever maps to a C++ struct."

**Repo:** `crates/cpp/include/dds/core/`

**Tests:** —

**Status:** `n/a (informative)` — Keine Klasse des PIM ist ein `struct`; `struct` steht
in den Headern nur für Eigenschaftsklassen (`smart_ptr_traits`, `is_topic_type`, …), die
`_def`-Strukturen der `safe_enum`, Hilfen in `detail` und die Erweiterungen `Enumerator`
und `BitField`.

---

## §7.4.2 Mapping Primitive and Container Types

### §7.4.2 Table 7.1: Boolean → `bool`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Boolean | bool"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `bool`. Der Test `check_table_7_1` prüft
mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen Typ
hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`); die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `std::vector<std::pair<K, V>>`,
`long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Char8 → `char`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Char8 | char"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `char`. Der Test `check_table_7_1` prüft
mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen Typ
hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`); die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `std::vector<std::pair<K, V>>`,
`long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Char32 → `wchar_t`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Char32 | wchar_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `wchar_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Byte → `uint8_t`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Byte | uint8_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `uint8_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int16 → `int16_t`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Int16 | int16_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `int16_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt16 → `uint16_t`

**Spec:** §7.4.2, S. 17, Table 7.1 — "UInt16 | uint16_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `uint16_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int32 → `int32_t`

**Spec:** §7.4.2, S. 17, Table 7.1 — "Int32 | int32_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `int32_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt32 → `uint32_t`

**Spec:** §7.4.2, S. 18, Table 7.1 — "UInt32 | uint32_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `uint32_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Int64 → `int64_t`

**Spec:** §7.4.2, S. 18, Table 7.1 — "Int64 | int64_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `int64_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: UInt64 → `uint64_t`

**Spec:** §7.4.2, S. 18, Table 7.1 — "UInt64 | uint64_t"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `uint64_t`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float64 → `double`

**Spec:** §7.4.2, S. 18, Table 7.1 — "Float64 | double"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `double`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float128 → `long double`

**Spec:** §7.4.2, S. 18, Table 7.1 — "Float128 | long double"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `long double`. Der Test `check_table_7_1`
prüft mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen
Typ hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`);
die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), `long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: Float32 → `float`

**Spec:** §7.4.2, S. 18, Table 7.1 — "Float32 | float"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Zeile der Tabelle ist `float`. Der Test `check_table_7_1` prüft
mit `static_assert`, dass der Zugriff auf das Mitglied eines erzeugten Typs diesen Typ
hat. Der Codegen erzeugt diese Typen in der Vorgabe (`--cpp-mapping psm`, commit `56ff5b5f25f`); die Option `--cpp-mapping native` behält die Typen des Codec (`map` als `std::vector<std::pair<K, V>>`,
`long double` als `::zerodds::wire::Float128`).

### §7.4.2 Table 7.1: string<Char8> → `std::string`

**Spec:** §7.4.2, S. 18, Table 7.1 — "string<Char8> | std::string"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::string` (`static_assert` in `check_table_7_1`; `std::string`
ist auch der Typ der Namen in den Headern).

### §7.4.2 Table 7.1: string<Char32> → `std::wstring`

**Spec:** §7.4.2, S. 18, Table 7.1 — "string<Char32> | std::wstring"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::wstring` in der Vorgabe (`--cpp-mapping psm`; `static_assert`); der Wert
läuft als UTF-8 durch den Codec (`back.wlabel() == r.wlabel()`).

### §7.4.2 Table 7.1: sequence<T> → `std::vector<T>`


**Spec:** §7.4.2, S. 18, Table 7.1 — "sequence<T> | std::vector<T>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`

**Status:** `done` — `std::vector<T>` in beiden Abbildungen (`static_assert`; Rust-Test
des Codegen).

### §7.4.2 Table 7.1: map<K, V> → `std::map<K, V>`

**Spec:** §7.4.2, S. 18, Table 7.1 — "map<K, V> | std::map<K, V>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — `std::map<K, V>` in der Vorgabe (`--cpp-mapping psm`; `static_assert`,
Roundtrip mit zwei Einträgen). Die Option `--cpp-mapping native` bleibt bei `zerodds::Map<K, V>` (ein `std::vector<std::pair<K, V>>`), weil die Reihenfolge der Einträge eine Achse des Codec ist
(`a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`).

### §7.4.2 Table 7.1: T[N] → `dds::core::array<T, N>`

**Spec:** §7.4.2, S. 18, Table 7.1 — "T[N] | dds::core::array<T, N>"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`,
`crates/cpp/include/dds/core/array.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`test_layout_headers_and_find`,
`test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::array<T, N>` in der Vorgabe (`--cpp-mapping psm`; `static_assert`,
mehrdimensional `cells()[1][2]`); die Option `--cpp-mapping native` benutzt `std::array`.
`dds::core::array` hat die Operationen von `std::array` (`fill`, `swap`, Iteratoren,
`get<I>`, `tuple_size`, Vergleiche).

### §7.4.2 Ganzzahltypen nach `stdint.h` von [C99]

**Spec:** §7.4.2, S. 18 — "The above fixed-size integer types shall conform to
the types of the same names as defined by [C99] in the header stdint.h."

**Repo:** `crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`test_layout_headers_and_find`

**Status:** `done` — Die Header binden `<stdint.h>` ein, wo sie die Typen global nennen
(commit `def4ff43f03`); der `static_assert` in `check_table_7_1` prüft die globalen
Namen.

### §7.4.2 Plattformen ohne [C99] liefern eigene Typen

**Spec:** §7.4.2, S. 18 — "The presence of these types shall not be construed to
require that DDS implementations only support [C99]-compliant platforms.
Implementations for non-[C99]-compliant platforms shall provide their own
conformant integer type definitions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis für Plattformen ohne [C99]; ZeroDDS verlangt
`<stdint.h>`.

### §7.4.2 Keine weiteren Definitionen aus `stdint.h` vorausgesetzt

**Spec:** §7.4.2, S. 18 — "It shall not be construed to imply the existence of
any other definitions that would be found in the header stdint.h on a
[C99]-compliant platform or even the existence of that header itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header benutzen aus `<stdint.h>` nur die
Ganzzahltypen fester Größe.

### §7.4.2 Typen im globalen Namensraum

**Spec:** §7.4.2, S. 18 — "Note that these types are defined in the global
namespace, not in the std namespace."

**Repo:** `crates/cpp/include/dds/core/types.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`

**Status:** `done` — Die Typen `int32_t` u. a. stehen im globalen Namensraum
(`static_assert(std::is_same<::int32_t, int32_t>…)`).

### §7.4.2 Begrenzte und unbegrenzte Folgen derselbe Typ

**Spec:** §7.4.2, S. 18 — "In addition, it is worth noticing that bounded and
unbounded sequence types map to the same C++ types."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::check_table_7_1`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Eine `sequence<long, 4>` und eine `sequence<long>` sind beide
`std::vector<int32_t>` (`static_assert`).

### §7.4.2 `dds::core::array` nach `std::array` ohne Verschieben

**Spec:** §7.4.2, S. 18 — "The DDS Array type is mapped to the dds::core::array
type which is specified to conform with the std::array type specified as part of
C++11, exception made for the move operators."

**Repo:** `crates/cpp/include/dds/core/array.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `dds::core::array` hat die Typen, Iteratoren, `fill`, `swap`, `at`,
`front`, `back`, `data`, die Vergleiche und das Tupel-Interface von `std::array`;
`std::array` ist es nicht als Alias (siehe §7.12).

---

## §7.4.3 Mapping Enumerations

### §7.4.3 Aufzählungen als sichere Klasse, unter C++11 `enum class`

**Spec:** §7.4.3, S. 18–19 — "Native enumerations in C++ are not safe. This
specification maps DDS enumerations to a safe enumeration class defined as
follows: [...] As such, for C++11 compilers, implementers may choose to map
enumeration to C++11 enumeration classes."

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp`,
`crates/cpp/include/dds/core/policy/PolicyKind.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — Die Kinds der Policies sind `safe_enum<…_def>`; die erzeugten
Aufzählungen sind `enum class` (C++11 erlaubt,
`an_enumeration_takes_an_underlying_type_from_its_bit_bound`).

### §7.4.3 `safe_enum`: Template und Basisklasse

**Spec:** §7.4.3, S. 18 — "namespace dds { namespace core { template<typename
def, typename inner = typename def::type> class safe_enum : public def { typedef
typename def::type type; inner val; [...]"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `safe_enum<def, inner>` erbt von `def`, so dass die Enumeratoren
als `Color::GREEN` erreichbar sind; ohne Umwandlung von und nach `int`.

### §7.4.3 `safe_enum`: Konstruktor aus `type`

**Spec:** §7.4.3, S. 18 — "safe_enum(type v) : val(v) {}"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::safe_enum`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `safe_enum(type v)`.

### §7.4.3 `safe_enum`: `underlying()`

**Spec:** §7.4.3, S. 18 — "inner underlying() const { return val; }"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::underlying`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `underlying()`.

### §7.4.3 `safe_enum`: `operator==` und `operator!=`

**Spec:** §7.4.3, S. 18 — "bool operator == (const safe_enum & s) const; bool
operator != (const safe_enum & s) const;"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::operator==`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — `operator==` und `operator!=`.

### §7.4.3 `safe_enum`: `operator<`, `<=`, `>`, `>=`

**Spec:** §7.4.3, S. 19 — "bool operator < (const safe_enum & s) const; bool
operator <= (const safe_enum & s) const; bool operator > (const safe_enum & s)
const; bool operator >= (const safe_enum & s) const;"

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp::safe_enum::operator<`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `operator<`, `<=`, `>`, `>=`.

### §7.4.3 Beispiel `Color` mit `Color_def`

**Spec:** §7.4.3, S. 19 — "Below we provide an example of how implementations of
this specification have to the safe_enum class to map DDS enumeration. [...]
enum Color_def__ { GREEN, WHITE, RED }; typedef dds::core::safe_enum<Color_def>
Color;"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel der Spec;
`test_value_and_reference_classes_without_other_coverage` benutzt es.

---

## §7.4.4 Mapping Unions

### §7.4.4 Unions nach IDL2C++11 §6.13.2

**Spec:** §7.4.4, S. 19 — "DDS unions mapping is the same as the one defined by
the IDL2C++11 specification as defined in 6.13.2 of the document ptc/2012-04-03.
This choice is compatible with the use of C++03 and aligns the mapping of DDS
types to that of IDL."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`, `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/cpp/tests/generated_psm_roundtrip.cpp::check_union_mapping`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** `done` — In der Vorgabe (`--cpp-mapping psm`) ist eine Union eine Klasse mit `_d()` und je Zweig
Zugriffs- und Änderungsmethode (`c.text("hello")` setzt Zweig und Diskriminator,
`holder.pick().text()`); eine Union mit `default`-Zweig, mit einem Member unter mehreren
Labels und mit implizitem Diskriminator ist im Roundtrip durch den Codec geprüft
(`check_union_mapping`: `static_assert`s und Vergleiche; Konvertierung liest nur den
gewählten Zweig). Verglichen ist mit IDL4 to C++ 1.0 §7.2.4.3.2, dem Nachfolger von
IDL2C++11 §6.13.2; ptc/2012-04-03 selbst liegt nicht im Cache und wurde nicht gelesen. Die Option `--cpp-mapping native` erzeugt eine `struct` mit öffentlichen Feldern.

---

## §7.4.5 Mapping Parameters Passing and Parameters Return Rules

### §7.4.5 Einfacher Typ, IN → `T`

**Spec:** §7.4.5, S. 19 — "PIM Native Type Parameter | DDS-PSM-Cxx Native Type
Parameter: IN T | T"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`

**Status:** `done` — Einfache Parameter gehen als Wert (`uint32_t domain_id`,
`History::KeepLast(uint32_t)`, `int32_t`-Setter).

### §7.4.5 Einfacher Typ, OUT → `T&`

**Spec:** §7.4.5, S. 19 — "OUT T | T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — Ein einfacher Rückgabewert über `T&` ist der Zugriff `int32_t&
depth()` der Policies; die Operationen der Header haben keinen OUT-Parameter einfachen
Typs (die Ergebnisse sind Rückgabewerte).

### §7.4.5 Einfacher Typ, INOUT → `T&`

**Spec:** §7.4.5, S. 19 — "INOUT T | T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — Wie bei OUT: `T&`-Zugriffe der Policies; keine Operation mit
INOUT-Parameter einfachen Typs.

### §7.4.5 Einfacher Typ, Rückgabe → `T`

**Spec:** §7.4.5, S. 20 — "PIM Native Return Type | DDS-PSM-Cxx Native Return
Type: T | T"

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::status_changes`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp::TDomainParticipant::domain_id`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — Einfache Rückgaben gehen als Wert (`domain_id()`,
`trigger_value()`, `contains_entity()`).

### §7.4.5 Zusammengesetzter Typ, IN → `const T&`

**Spec:** §7.4.5, S. 20 — "PIM Type Parameter | DDS-PSM-Cxx Type Parameter: IN T
| const T&"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::TTopic`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`

**Status:** `done` — Samples, Namen, Handles, QoS und Dauern gehen als `const T&`; die
Konstruktoren und Setter der Policies nehmen `const Duration&`, `const ByteSeq&`, `const
StringSeq&`.

### §7.4.5 Policies nehmen zusammengesetzte Typen als `const T&`

**Spec:** §7.4.5, S. 20 — "IN T | const T&"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Die Policies nehmen zusammengesetzte Typen als `const T&`
(`Reliability(kind, const Duration&)`, `Deadline(const Duration&)`, `UserData(const
ByteSeq&)`, `Partition(const StringSeq&)`). Die Zeile der Spec nennt `const T&`, nicht
den Wert.

### §7.4.5 Zusammengesetzter Typ, OUT → `T&`

**Spec:** §7.4.5, S. 20 — "OUT T | T&"

**Repo:** `crates/cpp/include/dds/core/cond/TWaitSet.hpp::TWaitSet::wait`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — OUT-Parameter zusammengesetzten Typs sind `T&` (`wait(ConditionSeq&
triggered, timeout)`, `read(iterator, max)` in einen Container).

### §7.4.5 Zusammengesetzter Typ, INOUT → `T&`

**Spec:** §7.4.5, S. 20 — "INOUT T | T&"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::key_value`,
`crates/cpp/include/dds/sub/TDataReader.hpp::TDataReader::key_value`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — INOUT-Parameter sind `T&` (`key_value(T& sample, handle)`).

### §7.4.5 Zusammengesetzter Typ, Rückgabe → `T` oder `const T&`

**Spec:** §7.4.5, S. 20 — "PIM Native Return Type | DDS-PSM-Cxx Native Return
Type: T | One of the following, depending on wether the return parameter is an
attribute or not. • T • const T&"

**Repo:** `crates/cpp/include/dds/sub/TSubscriber.hpp::TSubscriber::qos`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::qos`,
`crates/cpp/include/dds/topic/TTopic.hpp::TTopic::qos`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — Zusammengesetzte Rückgaben sind `T` oder `const T&` (`qos()` als
`const Qos&`, `default_writer_qos()` als Wert, Statuswerte als `const Status`).

---

## §7.4.6 Mapping Attributes

### §7.4.6 Attribut als Zustand, Getter und Setter, Konstruktorargument

**Spec:** §7.4.6, S. 20 — "Attributes defined by DDS PIM classes have to be
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

**Status:** `done` — Policies, Status, `SampleInfo`, `Time`, `Duration` und die Daten
der eingebauten Topics halten ihren Zustand privat, haben Getter und Setter mit dem
Namen des Attributs und Konstruktoren mit den Attributen.

### §7.4.6 Zugriffsfunktionen statt öffentlicher Felder

**Spec:** §7.4.6, S. 20 — "Getter/Setter methods shall be declared as described
in the following table."

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — Die Statusklassen, `SampleInfo`, die QoS-Mengen (`policy<P>()`) und
die Daten der eingebauten Topics haben keine öffentlichen Felder, sondern
Zugriffsfunktionen. Öffentliche Felder haben nur `Enumerator` und `BitField`
(Erweiterungen).

### §7.4.6 Einfacher Typ: `NT attribute();` und `void attribute(NT attrib);`

**Spec:** §7.4.6, S. 20 — "NT attribute; Where NT is a native. | NT attribute();
void attribute(NT attrib);"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/Time.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `NT attribute() const`, `NT& attribute()` und der Setter
`attribute(NT)`; die Policies geben mit dem Setter sich selbst zurück (verkettbar),
sonst `void`.

### §7.4.6 Zusammengesetzter Typ: `CT&`, `const CT&`, `void attribute(const CT&)`

**Spec:** §7.4.6, S. 20 — "CT attribute; Where CT is a constructed type (e.g., a
struct) | CT& attribute(); const CT& attribute() const; void attribute(const CT&
attrib);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/topic/TBuiltinTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_data_carries_every_member`

**Status:** `done` — `CT& attribute()`, `const CT& attribute() const` und der Setter mit
`const CT&` (Policies: `Reliability::max_blocking_time(const Duration&)`,
`UserData::value(const ByteSeq&)`).

### §7.4.6 Folgetyp: `ST&`, `const ST&`, `void attribute(const ST&)`

**Spec:** §7.4.6, S. 20 — "ST attribute; Where ST is a sequence type (e.g., a
string, sequence, map, arrays, etc.) | ST& attribute(); const ST& attribute()
const; void attribute(const ST& attrib);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — Folgen (`Partition::name`, `UserData::value`, `TopicData::value`)
haben `ST&`, `const ST&` und den Setter mit `const ST&`.

---

## §7.5 Core Package

### §7.5 Inhalt des Pakets core

**Spec:** §7.5, S. 21 — "The core package of the ISO/IEC C++ PSM for DDS
(DDS-PSM-Cxx) defines the classes at the foundation of the API object model as
well as all the DDS types used by all other modules. This sub clause describes
the most important classes of the package."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §7.5; jede Regel hat ein eigenes Item.

---

## §7.5.1 Object Model

### §7.5.1 Zwei Arten von Typen: Referenz und Wert

**Spec:** §7.5.1, S. 21 — "The ISO/IEC C++ PSM for DDS (DDS-PSM-Cxx) is based on
an object model that is structured in two different kinds of object types:
reference-types and value-types."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`,
`crates/cpp/include/dds/core/Value.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Referenztypen leiten von `Reference<DELEGATE>` ab, Werttypen von
`Value<D>`.

---

## §7.5.1.1 Reference Types

### §7.5.1.1 Flache Zuweisung einer Referenz

**Spec:** §7.5.1.1, S. 21 — "All objects that have a reference-type have an
associated shallow (polymorphic) assignment operator that simply changes the
value of the reference."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::operator=`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Kopie und Zuweisung einer Referenz teilen den Delegate
(`shared_ptr`); `test_reference_types` prüft, dass eine kopierte Referenz dasselbe
Objekt ist und dass die Zuweisung von `null` nur diese Referenz löst.

### §7.5.1.1 Referenzen sind sicher: null oder gültig

**Spec:** §7.5.1.1, S. 21 — "Furthermore reference-types are safe, meaning that
under no circumstances can a reference point to an invalid object. At any single
point in time a reference can either refer to the null object or to a valid
object."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp::Reference::require_delegate`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Eine Referenz ist null oder gültig; ein Aufruf über eine
Null-Referenz (`delegate()`, `operator->`, jede Operation der Klassen) ist
`NullReferenceError`, kein Zugriff über einen Nullzeiger.

### §7.5.1.1 Semantik durch `dds::core::Reference`

**Spec:** §7.5.1.1, S. 21 — "The semantics for Reference types is defined by the
DDS-PSM-Cxx class dds::core::Reference. In the context of this specification the
semantics implied by the ReferenceType is mandatory, yet the implementation
provided as part of this standard is provided to show one possible way of
implementing this semantics."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `dds::core::Reference<DELEGATE>` legt die Semantik fest; alle
Referenztypen der Table 7.2 leiten davon ab (`static_assert` in `test_reference_types`).

### §7.5.1.1 Referenztypen als Templates über den Delegate

**Spec:** §7.5.1.1, S. 21 — "All DDS-PSM-Cxx reference-types store references to
a delegate. [...] all DDS-PSM-Cxx reference-types are template classes whose
parameter is the DELEGATE. Each vendor will plug-in his implementation simply by
providing a file that instantiates the DDS-PSM-Cxx API with its own delegates."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/domain/TDomainParticipant.hpp`,
`crates/cpp/include/dds/pub/TPublisher.hpp`,
`crates/cpp/include/dds/sub/TSubscriber.hpp`, `crates/cpp/include/dds/topic/TTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Die Referenztypen sind Templates über den Delegate
(`TDomainParticipant<DELEGATE>` usw., commits `ee3c3473721`, `0c023caded7`); `Any*`
halten einen Holder.

### §7.5.1.1 Schwache Referenzen

**Spec:** §7.5.1.1, S. 21 — "Finally, the PSM also provides weak references."

**Repo:** `crates/cpp/include/dds/core/WeakReference.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `WeakReference<T>` mit `lock()` und `expired()`; `lock()` baut für
`DomainParticipant` und `DataWriter<T>` aus der Delegate-Referenz (Konstruktoren
`DomainParticipant(const DELEGATE_REF_T&)`, `DataWriter(const DELEGATE_REF_T&)`).

### §7.5.1.1 Table 7.2: `dds::core` Entity

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • Entity"

**Repo:** `crates/cpp/include/dds/core/Entity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::Entity` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` Condition

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • Condition"

**Repo:** `crates/cpp/include/dds/core/cond/Condition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::Condition` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` GuardCondition

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • GuardCondition"

**Repo:** `crates/cpp/include/dds/core/cond/GuardCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::GuardCondition` ist ein Referenztyp: `static_assert`
in `test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung,
Vergleich und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` ReadCondition

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • ReadCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::cond::ReadCondition` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` QueryCondition

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • QueryCondition"

**Repo:** `crates/cpp/include/dds/sub/cond/ReadCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::cond::QueryCondition` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::core` Waitset

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "dds | core | • Waitset"

**Repo:** `crates/cpp/include/dds/core/cond/WaitSet.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `core::cond::WaitSet` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::domain` DomainParticipant

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "pomain | • DomainParticipant"

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `domain::DomainParticipant` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` AnyDataWriter

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "pub | • AnyDataWriter"

**Repo:** `crates/cpp/include/dds/pub/AnyDataWriter.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::AnyDataWriter` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` Publisher

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "pub | • Publisher"

**Repo:** `crates/cpp/include/dds/pub/Publisher.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::Publisher` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::pub` DataWriter

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "pub | • DataWriter"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `pub::DataWriter` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` AnyDataReader

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "sub | • AnyDataReader"

**Repo:** `crates/cpp/include/dds/sub/AnyDataReader.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::AnyDataReader` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` Subscriber

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "sub | • Subscriber"

**Repo:** `crates/cpp/include/dds/sub/Subscriber.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::Subscriber` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` DataReader

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "sub | • DataReader"

**Repo:** `crates/cpp/include/dds/sub/DataReader.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::DataReader` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::sub` SharedSamples

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "sub | • SharedSamples"

**Repo:** `crates/cpp/include/dds/sub/SharedSamples.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `sub::SharedSamples` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::topic` AnyTopic

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "topic | • AnyTopic"

**Repo:** `crates/cpp/include/dds/topic/AnyTopic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `topic::AnyTopic` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

### §7.5.1.1 Table 7.2: `dds::topic` Topic

**Spec:** §7.5.1.1, S. 22, Table 7.2 — "topic | • Topic"

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `topic::Topic` ist ein Referenztyp: `static_assert` in
`test_reference_types`, dass er von `Reference<DELEGATE>` ableitet; Zuweisung, Vergleich
und Konstruktion mit `dds::core::null`.

---

## §7.5.1.2 Resource for Reference Types

### §7.5.1.2 Erzeugen über Konstruktoren

**Spec:** §7.5.1.2, S. 22 — "Instances of reference types are created using C++
constructors."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`,
`crates/cpp/include/dds/pub/Publisher.hpp`, `crates/cpp/include/dds/sub/Subscriber.hpp`,
`crates/cpp/include/dds/topic/Topic.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — Referenzen entstehen über Konstruktoren (`DomainParticipant(id)`,
`Publisher(dp)`, `Topic<T>(dp, name)` …).

### §7.5.1.2 Kein Standardkonstruktor für Referenztypen

**Spec:** §7.5.1.2, S. 22 — "The trivial constructor is not defined for
reference types, the only alternative to properly constructing a reference is to
initialize it to a null reference by assigning dds::core::null."

**Repo:** `crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `DomainParticipant`, `Topic`, `AnyTopic`, `ContentFilteredTopic`, `MultiTopic`, `Publisher`, `DataWriter`, `Subscriber` und `DataReader` (und ihre `T`-Basen) haben keinen öffentlichen Standardkonstruktor; die Null-Referenz entsteht aus `dds::core::null` (`static_assert` in `test_reference_types`). `WaitSet` und `GuardCondition` erzeugen beim Standardkonstruktor ein neues Objekt, wie die OMG-Header.

### §7.5.1.2 Null-Referenz über `dds::core::null`

**Spec:** §7.5.1.2, S. 22 — "the only alternative to properly constructing a
reference is to initialize it to a null reference by assigning dds::core::null."

**Repo:** `crates/cpp/include/dds/core/types.hpp::null`,
`crates/cpp/include/dds/core/Reference.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — Jeder Referenztyp lässt sich aus `dds::core::null` konstruieren,
ihr zuweisen und mit ihr vergleichen (`==`, `!=`).

### §7.5.1.2 `close()` beendet die Kommunikation und gibt Ressourcen frei

**Spec:** §7.5.1.2, S. 22 — "These objects therefore provide a method close()
that shall halt network communication (in the case of entities) and dispose of
any appropriate operating-system resources."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::close`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `close()` löscht den Writer: der Reader sieht ihn gehen
(`subscription_matched_status().current_count()` wird 0), ein späterer Aufruf ist
`AlreadyClosedError`; bei einem `retain()`-ten Objekt beendet es die Haltung.

### §7.5.1.2 Empfehlung, `close` aufzurufen

**Spec:** §7.5.1.2, S. 22 — "Users of this PSM are recommended to call close on
objects of all reference types once they are finished using them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung an die Anwender; keine Anforderung an die
Implementierung.

### §7.5.1.2 Direkt referenziertes Objekt bleibt in Gebrauch

**Spec:** §7.5.1.2, S. 22 — "In addition, implementations may automatically
close objects that they deem to be no longer in use, subject to the following
restrictions: • Any object to which the application has a direct reference (not
including a WeakRefer- ence) is still in use."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — Ein Writer und ein Reader, die die Anwendung hält, bleiben
benutzbar, nachdem Participant, Topic, Publisher und Subscriber ihren Gültigkeitsbereich
verlassen haben: Kinder halten ihre Eltern (`shared_ptr`, commit `9de21b2827b`). Der
Test schickt danach ein Sample.

### §7.5.1.2 Entität mit Listener bleibt in Gebrauch

**Spec:** §7.5.1.2, S. 22 — "• Any entity with a non-null listener is still in
use."

**Repo:** `crates/cpp/include/dds/core/detail/Retain.hpp::retain_for_listener`,
`crates/cpp/include/dds/core/TEntity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_table_is_per_entity_and_deregisters_on_destroy`

**Status:** `done` — `listener(l, mask)` mit einem Listener hält die Entität
(Participant, Topic, Publisher, Subscriber, DataWriter, DataReader) über die letzte
Referenz hinaus, `listener(nullptr, …)` gibt sie frei.

### §7.5.1.2 Ausdrücklich gehaltenes Objekt bleibt in Gebrauch

**Spec:** §7.5.1.2, S. 22 — "• Any object that has been explicitly retained is
still in use"

**Repo:** `crates/cpp/include/dds/core/detail/Retain.hpp`,
`crates/cpp/include/dds/core/TEntity.hpp::TEntity::retain`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`

**Status:** `done` — `retain()` hält die Entität bis `close()`; ein `retain()`-ter
Participant wird mit `dds::domain::find` gefunden, auch ohne Referenz der Anwendung.

### §7.5.1.2 Ersteller einer benutzten Entität bleibt in Gebrauch

**Spec:** §7.5.1.2, S. 22 — "• The creator of any object that is still in use is
itself still in use."

**Repo:** `crates/cpp/include/dds/pub/detail/DataWriterDelegate.hpp`,
`crates/cpp/include/dds/sub/detail/DataReaderDelegate.hpp`,
`crates/cpp/include/dds/topic/detail/TopicDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — Writer und Reader halten Publisher, Subscriber und Topic, diese
halten das Handle des Participant (`shared_ptr` mit dem Deleter des C-API): das Handle
wird mit dem letzten Kind freigegeben (commit `9de21b2827b`).

---

## §7.5.2 Value Types

### §7.5.2 Kopie und Zuweisung von Werttypen tief

**Spec:** §7.5.2, S. 23 — "All objects that have a value-type have a deep-copy
assignment and copy construction semantics."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Kopie und Zuweisung einer QoS-Menge, einer Policy, eines Status und
von `Time` und `Duration` sind tief; der Test ändert die Kopie und prüft das Original.

### §7.5.2 Werttypen über `dds::core::Value`

**Spec:** §7.5.2, S. 23; §7.5.4, S. 23; §7.6.1, S. 26 — "class SampleLostStatus
: public dds::core::Value<D>"; "class THistory : public dds::core::Value<D>"

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `TSampleLostStatus<D>`, `THistory<D>` usw. leiten von `Value<D>` ab
(commit `0c023caded7`).

### §7.5.2 Werttypen sind änderbar

**Spec:** §7.5.2, S. 23 — "It should also be pointed out that value-types are
not “pure-value-types” in the sense that they are immutable (as in functional
programming languages). The DDS-PSM-Cxx makes value-types mutable to limit the
number of copies as well limit the time-overhead necessary to change a
value-type"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Setter ändern Policies und Status.

### §7.5.2 Alle Klassen außerhalb von Table 7.2 sind Werttypen

**Spec:** §7.5.2, S. 23 — "The DDS-PSM-Cxx models all DDS PIM classes beyond
what is listed in Table 7.2 as value- types. In other terms, QoS, Policy,
Statuses, and Topic samples are all modeled as value-types."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/sub/SampleInfo.hpp`, `crates/cpp/include/dds/sub/Sample.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_matched_data_names_the_other_endpoint`

**Status:** `done` — QoS, Policies, Status, `SampleInfo`, `Sample<T>`, `Time`,
`Duration`, `InstanceHandle` und die erzeugten Topic-Typen sind kopierbar. Für die
Klassen, die Table 7.2 nicht nennt (ContentFilteredTopic, StatusCondition,
TopicDescription) siehe das folgende Item.

### §7.5.2 ContentFilteredTopic und StatusCondition als Werttypen

**Spec:** §7.5.2, S. 23; §7.5.1.1, S. 22, Table 7.2 — "The DDS-PSM-Cxx models
all DDS PIM classes beyond what is listed in Table 7.2 as value- types."

**Repo:** `crates/cpp/include/dds/topic/TContentFilteredTopic.hpp`,
`crates/cpp/include/dds/core/cond/TStatusCondition.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_contentfilteredtopic_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_find_ignore_status_condition`

**Status:** `done` — Table 7.2 nennt `ContentFilteredTopic`, `StatusCondition` und
`TopicDescription` nicht; die OMG-Header (die nach §2 vorgehen) leiten sie von
`Reference` ab (`TContentFilteredTopic : TopicDescription : Reference`,
`TStatusCondition : TCondition`). ZeroDDS folgt den Headern: Referenztypen, kopierbar
mit geteiltem Delegate.

---

## §7.5.3 Any Types

### §7.5.3 Wenige virtuelle Methoden, Delegation statt Vererbung

**Spec:** §7.5.3, S. 23 — "The DDS-PSM-Cxx has been designed to take advantage
of the compile time polymorphism provided by C++ templates. As such, the whole
standard interface only has a few virtual methods, and in general does not rely
on inheritance but as opposed exploits delegation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Entwurfsziel. In ZeroDDS sind die Methoden der
Listener und der Destruktor von `Condition` virtuell.

### §7.5.3 Typen „Any“ für Entitäten ohne vollständigen Typ

**Spec:** §7.5.3, S. 23 — "Since the DDS API requires at times to pass DDS
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

**Status:** `done` — `AnyDataWriter`, `AnyDataReader`, `AnyTopic` und
`AnyTopicDescription` halten eine Entität beliebigen Typs; `get<T>()` gibt sie zurück,
ein anderer Typ ist `InvalidDowncastError`. `AnyDataWriter`, `AnyDataReader` und
`AnyTopic` sind Referenztypen (Table 7.2).

---

## §7.5.4 Status Classes

### §7.5.4 Statusklassen nach §7.4 ohne gemeinsame Basis

**Spec:** §7.5.4, S. 23 — "The DDS-PSM-Cxx mapping for the status classes as
defined in the DDS v1.2 specification is obtained by applying the generic
mapping rules described in 7.4 with the following caption – inheritance from the
root status class has been ignored."

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Die Statusklassen haben keine gemeinsame Basisklasse; jede ist
`TXStatus<D> : Value<D>`.

### §7.5.4 Namensraum `dds::core::status`

**Spec:** §7.5.4, S. 23 — "Status classes are part of the dds::core::status
namespace."

**Repo:** `crates/cpp/include/dds/core/status/Status.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Die Statusklassen stehen in `dds::core::status`.

### §7.5.4 `SampleLostStatus` als Template über `Value<D>`

**Spec:** §7.5.4, S. 23 — "namespace dds { namespace core { namespace status {
template <typename D> class SampleLostStatus : public dds::core::Value<D>{"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `TSampleLostStatus<D> : Value<D>`;
`dds::core::status::SampleLostStatus` ist `TSampleLostStatus<detail::SampleLostStatus>`.

### §7.5.4 `SampleLostStatus`: Konstruktoren

**Spec:** §7.5.4, S. 23 — "SampleLostStatus(); SampleLostStatus(uint32_t
total_count, uint32_t total_count_change);"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Der Standardkonstruktor und `SampleLostStatus(total_count,
total_count_change)`. Die PDF schreibt `uint32_t`, die OMG-Header `int32_t`; ZeroDDS
folgt den Headern (`int32_t`).

### §7.5.4 `SampleLostStatus`: Zugriff auf `total_count`

**Spec:** §7.5.4, S. 24 — "uint32_t total_count() const; uint32_t&
total_count(); void total_count(uint32_t total_count);"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus::total_count`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `total_count() const`, `total_count()` als Referenz und
`total_count(value)`.

### §7.5.4 Alle Statusklassen in `dds/core/status/Status.hpp`

**Spec:** §7.5.4, S. 24 — "The full set of status classes is includes in the
mandatory standard headers in the file dds/core/status/Status.hpp."

**Repo:** `crates/cpp/include/dds/core/status/Status.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Alle elf Statusklassen stehen in `dds/core/status/Status.hpp`.

### §7.5.4 StatusKind und StatusMask

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159 (`dds-1.2.pdf`) — "typedef
unsigned long StatusKind; typedef unsigned long StatusMask; const StatusKind
INCONSISTENT_TOPIC_STATUS = 0x0001 << 0; [...] const StatusKind
SUBSCRIPTION_MATCHED_STATUS = 0x0001 << 14;"

**Repo:** `crates/cpp/include/dds/core/status/State.hpp::StatusMask`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`

**Status:** `done` — `StatusMask` mit den Bitwerten der IDL von DDS 1.2
(`data_available() == 1 << 10` usw.), `all()`, `none()` und `get_status<STATUS>()` je
Statusklasse.

### §7.5.4 InconsistentTopicStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159 (`dds-1.2.pdf`) — "struct
InconsistentTopicStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TInconsistentTopicStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `InconsistentTopicStatus` hat die Mitglieder der IDL (total_count,
total_count_change) als Zugriffsfunktionen; der Test baut sie aus Werten und liest sie.

### §7.5.4 SampleLostStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159 (`dds-1.2.pdf`) — "struct
SampleLostStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `SampleLostStatus` hat die Mitglieder der IDL (total_count,
total_count_change) als Zugriffsfunktionen; der Test baut sie aus Werten und liest sie.

### §7.5.4 SampleRejectedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159 (`dds-1.2.pdf`) — "enum
SampleRejectedStatusKind { NOT_REJECTED, REJECTED_BY_INSTANCES_LIMIT,
REJECTED_BY_SAMPLES_LIMIT, REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT }; struct
SampleRejectedStatus { long total_count; long total_count_change;
SampleRejectedStatusKind last_reason; InstanceHandle_t last_instance_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSampleRejectedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `SampleRejectedStatus` hat die Mitglieder der IDL (total_count,
total_count_change, last_reason, last_instance_handle) als Zugriffsfunktionen; der Test
baut sie aus Werten und liest sie.

### §7.5.4 LivelinessLostStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159 (`dds-1.2.pdf`) — "struct
LivelinessLostStatus { long total_count; long total_count_change; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TLivelinessLostStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `LivelinessLostStatus` hat die Mitglieder der IDL (total_count,
total_count_change) als Zugriffsfunktionen; der Test baut sie aus Werten und liest sie.

### §7.5.4 LivelinessChangedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 159–160 (`dds-1.2.pdf`) — "struct
LivelinessChangedStatus { long alive_count; long not_alive_count; long
alive_count_change; long not_alive_count_change; InstanceHandle_t
last_publication_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TLivelinessChangedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `LivelinessChangedStatus` hat die Mitglieder der IDL (alive_count,
not_alive_count, alive_count_change, not_alive_count_change, last_publication_handle)
als Zugriffsfunktionen; der Test baut sie aus Werten und liest sie.

### §7.5.4 OfferedDeadlineMissedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
OfferedDeadlineMissedStatus { long total_count; long total_count_change;
InstanceHandle_t last_instance_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TOfferedDeadlineMissedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `OfferedDeadlineMissedStatus` hat die Mitglieder der IDL
(total_count, total_count_change, last_instance_handle) als Zugriffsfunktionen; der Test
baut sie aus Werten und liest sie.

### §7.5.4 RequestedDeadlineMissedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
RequestedDeadlineMissedStatus { long total_count; long total_count_change;
InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TRequestedDeadlineMissedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `RequestedDeadlineMissedStatus` hat die Mitglieder der IDL
(total_count, total_count_change, last_instance_handle) als Zugriffsfunktionen; der Test
baut sie aus Werten und liest sie.

### §7.5.4 OfferedIncompatibleQosStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
QosPolicyCount { QosPolicyId_t policy_id; long count; }; struct
OfferedIncompatibleQosStatus { long total_count; long total_count_change;
QosPolicyId_t last_policy_id; QosPolicyCountSeq policies; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TOfferedIncompatibleQosStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `OfferedIncompatibleQosStatus` hat die Mitglieder der IDL
(total_count, total_count_change, last_policy_id, policies) als Zugriffsfunktionen; der
Test baut sie aus Werten und liest sie.

### §7.5.4 RequestedIncompatibleQosStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
RequestedIncompatibleQosStatus { long total_count; long total_count_change;
QosPolicyId_t last_policy_id; QosPolicyCountSeq policies; };"

**Repo:**
`crates/cpp/include/dds/core/status/TStatus.hpp::TRequestedIncompatibleQosStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `RequestedIncompatibleQosStatus` hat die Mitglieder der IDL
(total_count, total_count_change, last_policy_id, policies) als Zugriffsfunktionen; der
Test baut sie aus Werten und liest sie.

### §7.5.4 PublicationMatchedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
PublicationMatchedStatus { long total_count; long total_count_change; long
current_count; long current_count_change; InstanceHandle_t
last_subscription_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TPublicationMatchedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `PublicationMatchedStatus` hat die Mitglieder der IDL (total_count,
total_count_change, current_count, current_count_change, last_subscription_handle) als
Zugriffsfunktionen; der Test baut sie aus Werten und liest sie.

### §7.5.4 SubscriptionMatchedStatus

**Spec:** §7.5.4, S. 23; [DDS] 1.2 IDL, S. 160–161 (`dds-1.2.pdf`) — "struct
SubscriptionMatchedStatus { long total_count; long total_count_change; long
current_count; long current_count_change; InstanceHandle_t
last_publication_handle; };"

**Repo:** `crates/cpp/include/dds/core/status/TStatus.hpp::TSubscriptionMatchedStatus`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`

**Status:** `done` — `SubscriptionMatchedStatus` hat die Mitglieder der IDL
(total_count, total_count_change, current_count, current_count_change,
last_publication_handle) als Zugriffsfunktionen; der Test baut sie aus Werten und liest
sie.

---

## §7.5.5 Error Codes

### §7.5.5 Table 7.3: RETCODE_OK

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_OK | Normal return; no exception"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_OK` ist keine Ausnahme: `check_status` wirft nichts;
`NO_DATA` ist eine leere Folge beim Lesen.

### §7.5.5 Table 7.3: RETCODE_NO_DATA

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_NO_DATA | An informational state
attached to a normal return; no exception"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_NO_DATA` ist keine Ausnahme: `check_status` wirft nichts;
`NO_DATA` ist eine leere Folge beim Lesen.

### §7.5.5 Table 7.3: RETCODE_ERROR

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_ERROR | Error | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::Error`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ERROR` wird `dds::core::Error`, abgeleitet von
`dds::core::Exception` und `std::exception` (`static_assert` und Wurf in
`test_return_codes_map_to_exceptions`). Die PDF nennt `std::logic_error` als
Elternklasse von `Error`, die OMG-Header `std::exception`; ZeroDDS folgt den Headern.

### §7.5.5 Table 7.3: RETCODE_BAD_PARAMETER

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_BAD_PARAMETER |
InvalidArgumentError | std::invalid_argument"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::InvalidArgumentError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_BAD_PARAMETER` wird `dds::core::InvalidArgumentError`,
abgeleitet von `dds::core::Exception` und `std::invalid_argument` (`static_assert` und
Wurf in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_TIMEOUT

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_TIMEOUT | TimeoutError |
std::runtime_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::TimeoutError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_TIMEOUT` wird `dds::core::TimeoutError`, abgeleitet von
`dds::core::Exception` und `std::runtime_error` (`static_assert` und Wurf in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_UNSUPPORTED

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_UNSUPPORTED | UnsupportedError |
std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::UnsupportedError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_UNSUPPORTED` wird `dds::core::UnsupportedError`,
abgeleitet von `dds::core::Exception` und `std::logic_error` (`static_assert` und Wurf
in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_ALREADY_DELETED

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_ALREADY_DELETED |
AlreadyClosedError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::AlreadyClosedError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ALREADY_DELETED` wird `dds::core::AlreadyClosedError`,
abgeleitet von `dds::core::Exception` und `std::logic_error` (`static_assert` und Wurf
in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_ILLEGAL_OPERATION

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_ILLEGAL_OPERATION |
IllegalOperationError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::IllegalOperationError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_ILLEGAL_OPERATION` wird
`dds::core::IllegalOperationError`, abgeleitet von `dds::core::Exception` und
`std::logic_error` (`static_assert` und Wurf in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_NOT_ENABLED

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_NOT_ENABLED | NotEnabledError |
std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::NotEnabledError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_NOT_ENABLED` wird `dds::core::NotEnabledError`, abgeleitet
von `dds::core::Exception` und `std::logic_error` (`static_assert` und Wurf in
`test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_PRECONDITION_NOT_MET

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_PRECONDITION_NOT_MET |
PreconditionNotMetError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::PreconditionNotMetError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_PRECONDITION_NOT_MET` wird
`dds::core::PreconditionNotMetError`, abgeleitet von `dds::core::Exception` und
`std::logic_error` (`static_assert` und Wurf in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_IMMUTABLE_POLICY

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_IMMUTABLE_POLICY |
ImmutablePolicyError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::ImmutablePolicyError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_IMMUTABLE_POLICY` wird `dds::core::ImmutablePolicyError`,
abgeleitet von `dds::core::Exception` und `std::logic_error` (`static_assert` und Wurf
in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_INCONSISTENT_POLICY

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_INCONSISTENT_POLICY |
InconsistentPolicyError | std::logic_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::InconsistentPolicyError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_INCONSISTENT_POLICY` wird
`dds::core::InconsistentPolicyError`, abgeleitet von `dds::core::Exception` und
`std::logic_error` (`static_assert` und Wurf in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: RETCODE_OUT_OF_RESOURCES

**Spec:** §7.5.5, S. 24, Table 7.3 — "RETCODE_OUT_OF_RESOURCES |
OutOfResourcesError | std::runtime_error"

**Repo:** `crates/cpp/include/zerodds/psm/check_status.hpp::check_status`,
`crates/cpp/include/dds/core/Exception.hpp::OutOfResourcesError`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — `RETCODE_OUT_OF_RESOURCES` wird `dds::core::OutOfResourcesError`,
abgeleitet von `dds::core::Exception` und `std::runtime_error` (`static_assert` und Wurf
in `test_return_codes_map_to_exceptions`).

### §7.5.5 Table 7.3: Elternklassen der Ausnahmen

**Spec:** §7.5.5, S. 24, Table 7.3 — "Std C++ Parent Exception: std::logic_error
(ERROR, UNSUPPORTED, ALREADY_DELETED, ILLEGAL_OPERATION, NOT_ENABLED,
PRECONDITION_NOT_MET, IMMUTABLE_POLICY, INCONSISTENT_POLICY),
std::invalid_argument (BAD_PARAMETER), std::runtime_error (TIMEOUT,
OUT_OF_RESOURCES)"

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Die Elternklassen stimmen mit der Tabelle überein
(`static_assert`), mit der Ausnahme von `Error` (siehe RETCODE_ERROR).

### §7.5.5 Ausnahmen in `dds::core` mit Basis `Exception`

**Spec:** §7.5.5, S. 24 — "The DDS-PSM-Cxx maps error codes to C++ exceptions
defined in the dds::core namespace and inheriting from a base Exception class
and the appropriate standard C++ exception."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp::Exception`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Jede Ausnahme steht in `dds::core`, leitet von
`dds::core::Exception` und von der Standardausnahme ab.

### §7.5.5 Ausnahmen haben Wertsemantik

**Spec:** §7.5.5, S. 24 — "Exceptions have value semantics, this means have to
always have deep copy semantics."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Kopierkonstruktor und Zuweisung kopieren die Nachricht tief; die
Kopie überlebt das Original.

### §7.5.5 Alle Ausnahmen in `dds/core/Exceptions.hpp`

**Spec:** §7.5.5, S. 24 — "The full list of exceptions is included in the file
dds/core/Exceptions.hpp."

**Repo:** `crates/cpp/include/dds/core/Exception.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_return_codes_map_to_exceptions`

**Status:** `done` — Alle Ausnahmen stehen in einer Datei. Die PDF nennt sie
`dds/core/Exceptions.hpp`, die OMG-Header `dds/core/Exception.hpp`; ZeroDDS folgt den
Headern.

---

## §7.5.6 Time and Duration

### §7.5.6 Time_t und Duration_t als Werttypen Time und Duration

**Spec:** §7.5.6, S. 24 — "This PSM maps the DDS Time_t and Duration_t types
into the value types Time and Duration respectively."

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `dds::core::Time` und `dds::core::Duration`.

### §7.5.6 Sekunden und Nanosekunden über Getter und Setter

**Spec:** §7.5.6, S. 24 — "In addition to providing their seconds and
nanoseconds state through accessor and mutator methods, these classes provide a
small number of convenience operations:"

**Repo:** `crates/cpp/include/dds/core/Time.hpp`,
`crates/cpp/include/dds/core/Duration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `sec()`, `nanosec()` als Getter und Setter; ein Setter für
`nanosec` über 999 999 999 normalisiert in die Sekunden.

### §7.5.6 Setter von Time und Duration

**Spec:** §7.5.6, S. 24 — "providing their seconds and nanoseconds state through
accessor and mutator methods"

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::sec`,
`crates/cpp/include/dds/core/Duration.hpp::Duration::nanosec`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time::sec(int64_t)`, `Time::nanosec(uint32_t)`,
`Duration::sec(int32_t)`, `Duration::nanosec(uint32_t)`; der Test prüft die
Normalisierung von `Duration::nanosec`.

### §7.5.6 Time um Dauern erhöhen

**Spec:** §7.5.6, S. 24 — "• Time object scan be incremented by durations
expressed as seconds, nanoseconds, milliseconds, or Duration objects."

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::operator+=`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time += Duration`, `Time + Duration`, `Time - Duration`.

### §7.5.6 Time aus und in Millisekunden

**Spec:** §7.5.6, S. 24 — "• Time object scan be converted to and from times
expressed in milliseconds (or other units) as integer types."

**Repo:** `crates/cpp/include/dds/core/Time.hpp::Time::from_millisecs`,
`crates/cpp/include/dds/core/Time.hpp::Time::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Time::from_millisecs`, `from_microsecs`, `from_secs` und
`to_millisecs`, `to_microsecs`, `to_secs`.

### §7.5.6 Duration um Dauern erhöhen

**Spec:** §7.5.6, S. 24 — "• Duration objects can be incremented by durations
expressed as seconds, nanoseconds, milliseconds, or Duration objects."

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::operator+=`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Duration += Duration`, `+`, `-`, `*`, `/`; ein unendlicher Operand
macht die Summe unendlich.

### §7.5.6 Duration aus und in Millisekunden

**Spec:** §7.5.6, S. 24 — "• Duration objects can be converted to and from
durations expressed in milliseconds(or other units) as integer types."

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::from_millisecs`,
`crates/cpp/include/dds/core/Duration.hpp::Duration::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `Duration::from_millisecs` (ein Wert über `int32_t`-Sekunden ist
`std::overflow_error`), `from_microsecs`, `from_secs` und `to_millisecs`,
`to_microsecs`, `to_secs`.

### §7.5.6 Duration in Millisekunden ausgeben

**Spec:** §7.5.6, S. 24 — "converted to and from durations expressed in
milliseconds(or other units) as integer types"

**Repo:** `crates/cpp/include/dds/core/Duration.hpp::Duration::to_millisecs`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_duration_sentinel_conversion_and_overflow`

**Status:** `done` — `to_millisecs()` und `to_microsecs()` liefern die Dauer als ganze
Zahl (`Duration(2, 500000000).to_millisecs() == 2500`); `wait_for_acknowledgments` und
`wait_for_matched` benutzen sie.

---

## §7.6 QoS Packages

### §7.6 Erweiterbare Policies und QoS ohne Änderung der API

**Spec:** §7.6, S. 25 — "The QoS package provides all definitions for Policy and
QoS.The DDS-PSM-Cxx provide extensible policy and extensible QoS. This means
that vendor can easily add additional attributes to policy as well as new
policies to Qos. All of this without requiring changes in the public API."

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`, `crates/cpp/include/dds/core/Value.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Policies sind Templates über einen Delegate (`THistory<D>` …),
QoS-Klassen Mengen von Policies (`TEntityQos<PolicySet<…>>`); Erweiterungen liegen im
Delegate (`history->…`, `history.delegate()`).

### §7.6 Erweiterungen über `operator->` und `delegate()`

**Spec:** §7.6, S. 25 — "As explained above, the PSM uses the “operator ->”, or
equivalently the “delegate()” method to access vendor-specific extensions."

**Repo:** `crates/cpp/include/dds/core/Value.hpp::Value::operator->`,
`crates/cpp/include/dds/core/Reference.hpp::Reference::operator->`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `operator->` und `delegate()` an Werttypen (`history->depth()`) und
an Referenztypen (`dp->find_topic(…)`).

### §7.6 Namensräume der QoS-Klassen

**Spec:** §7.6.2.1, S. 27; §7.13.1, S. 30; §7.2, S. 15, Figure 7.1 —
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

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantFactoryQos { EntityFactoryQosPolicy entity_factory; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Header der OMG haben weder
`DomainParticipantFactoryQos` noch eine Factory (siehe „DomainParticipantFactory:
get_qos und set_qos“); ZeroDDS hat die Klasse ebenfalls nicht (eine Erweiterung
in `dds::` wäre nach §2.2 nicht zulässig).

### §7.6 DomainParticipantQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantQos { UserDataQosPolicy user_data; EntityFactoryQosPolicy
entity_factory; };"

**Repo:** `crates/cpp/include/dds/domain/qos/DomainParticipantQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DomainParticipantQos` enthält `UserData` und `EntityFactory`, wie
die IDL.

### §7.6 TopicQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 167–168 (`dds-1.2.pdf`) — "struct
TopicQos { topic_data; durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; ownership; };"

**Repo:** `crates/cpp/include/dds/topic/qos/TopicQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`

**Status:** `done` — `TopicQos` enthält die 13 Policies der IDL; der Test prüft die
Vorgaben von RELIABILITY und RESOURCE_LIMITS.

### §7.6 PublisherQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 168 (`dds-1.2.pdf`) — "struct
PublisherQos { PresentationQosPolicy presentation; PartitionQosPolicy partition;
GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory; };"

**Repo:** `crates/cpp/include/dds/pub/qos/PublisherQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `PublisherQos` enthält Presentation, Partition, GroupData und
EntityFactory, wie die IDL.

### §7.6 SubscriberQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 169 (`dds-1.2.pdf`) — "struct
SubscriberQos { PresentationQosPolicy presentation; PartitionQosPolicy
partition; GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory;
};"

**Repo:** `crates/cpp/include/dds/sub/qos/SubscriberQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `SubscriberQos` enthält Presentation, Partition, GroupData und
EntityFactory, wie die IDL.

### §7.6 DataWriterQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 168 (`dds-1.2.pdf`) — "struct
DataWriterQos { durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; user_data; ownership; ownership_strength;
writer_data_lifecycle; };"

**Repo:** `crates/cpp/include/dds/pub/qos/DataWriterQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DataWriterQos` enthält die 15 Policies der IDL, RELIABILITY steht
auf RELIABLE; eine QoS ändert sich mit `qos(q)` am laufenden Writer.

### §7.6 DataReaderQos

**Spec:** §7.6, S. 25; [DDS] 1.2 IDL, S. 168–169 (`dds-1.2.pdf`) — "struct
DataReaderQos { durability; deadline; latency_budget; liveliness; reliability;
destination_order; history; resource_limits; user_data; ownership;
time_based_filter; reader_data_lifecycle; };"

**Repo:** `crates/cpp/include/dds/sub/qos/DataReaderQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_qos_of_a_running_entity_is_set_and_read`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_are_created_with_qos_listener_and_mask`

**Status:** `done` — `DataReaderQos` enthält die 12 Policies der IDL, RELIABILITY steht
auf BEST_EFFORT, HISTORY auf `KEEP_LAST` mit `depth` 1.

---

## §7.6.1 Policy Classes

### §7.6.1 Policies nach §7.4 ohne gemeinsame Basis

**Spec:** §7.6.1, S. 25 — "The DDS-PSM-Cxx mapping for the policy classes as
defined in the DDS v1.2 specification is obtained by applying the generic
mapping rules described in 7.4 with the following guidelines: • the inheritance
from the root Policy class has been ignored"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Die Policy-Klassen haben keine gemeinsame Basisklasse.

### §7.6.1 Namen ohne „QosPolicy“

**Spec:** §7.6.1, S. 25 — "• the trailing “QosPolicy” has to be discarded from
the name as redundant."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Die Namen sind `UserData`, `History`, … ohne `QosPolicy`.

### §7.6.1 Art als Aufzählung mit Hilfsfunktionen je Variante

**Spec:** §7.6.1, S. 25–26 — "• Policy kind is represented with a C++
enumeration and an associated constructor type as shown in the example below.
[...] first it associates an enumeration with the Policy defining a code for the
variation (as it was done in the IDL PSM), then, it defines a set of helper
methods to construct the possible variants."

**Repo:** `crates/cpp/include/dds/core/policy/PolicyKind.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Jede Art ist ein `safe_enum` (`HistoryKind`, `ReliabilityKind`, …)
mit einer statischen Hilfsfunktion je Variante (`History::KeepAll()`,
`Reliability::BestEffort()`, `Durability::Persistent()`, `Liveliness::ManualByTopic(d)`,
`Ownership::Exclusive()`, `DestinationOrder::SourceTimestamp()`,
`Presentation::TopicAccessScope(…)`).

### §7.6.1 Fehlende Hilfsfunktionen und `Reliable_`

**Spec:** §7.6.1, S. 26; §7.8, S. 28; §7.13.1, S. 30 — "tqos <<
Reliability::Reliable() << Ownership::Exclusive();"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::TReliability::Reliable`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `tqos << Reliability::Reliable() << Ownership::Exclusive()`
(Beispiel der Spec) übersetzt und wirkt; `Reliable()` hat einen Parameter für die
maximale Blockierzeit.

### §7.6.1 Policies im Namensraum `dds::qos`

**Spec:** §7.6.1, S. 25 — "Policy classes are part of the dds::qos namespace"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`

**Status:** `done` — Die PDF nennt `dds::qos`, die OMG-Header `dds::core::policy`;
ZeroDDS folgt den Headern (`dds::core::policy`).

### §7.6.1 Name und ID über Traits

**Spec:** §7.6.1, S. 25 — "the Policy Name and Policy ID are to be provided by
specialization of the following trait classes:"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_id<P>::value` und `policy_name<P>::name()` sind für alle 22
Policies definiert (ID nach DDS 1.2); `OMG_DDS_POLICY_TRAITS` und
`OMG_DDS_DEFINE_POLICY_TRAITS` stehen für eigene Policies bereit.

### §7.6.1 Trait `policy_id` mit `id = -1`

**Spec:** §7.6.1, S. 25 — "namespace dds { namespace qos { template <typename
Policy> class policy_id { public: enum { id = -1 }; };"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_id` ist ein Klassen-Template, spezialisiert je Policy mit
`value`. Die PDF schreibt `enum { id = -1 }` im Primärtemplate, die OMG-Header ein
leeres Template mit `value` in den Spezialisierungen; ZeroDDS folgt den Headern.

### §7.6.1 Trait `policy_name`

**Spec:** §7.6.1, S. 25 — "template <typename Policy> class policy_name { }; }
}"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `policy_name<P>::name()` gibt den Namen der Policy.

### §7.6.1 `HistoryKind_def` und `HistoryKind` als `safe_enum`

**Spec:** §7.6.1, S. 25–26 — "namespace dds { namespace qos { struct
HistoryKind_def { enum Type { KEEP_LAST, KEEP_ALL }; }; typedef
dds::core::safe_enum<HistoryKind_def> HistoryKind; }}"

**Repo:** `crates/cpp/include/dds/core/policy/PolicyKind.hpp::HistoryKind_def`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `HistoryKind_def { enum type { KEEP_LAST, KEEP_ALL }; }` und
`safe_enum<HistoryKind_def> HistoryKind`; sie stehen in `dds::core::policy`.

### §7.6.1 `THistory` als Template über `Value<D>`

**Spec:** §7.6.1, S. 26 — "namespace dds { namespace qos { template <typename D>
class THistory : public dds::core::Value<D> {"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `THistory<D> : Value<D>`.

### §7.6.1 `THistory`: Konstruktoren

**Spec:** §7.6.1, S. 26 — "THistory(); THistory(HistoryKind kind, int32_t
depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::THistory`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `THistory()` und `THistory(kind, depth)`.

### §7.6.1 `THistory`: Zugriff auf `kind`

**Spec:** §7.6.1, S. 26 — "HistoryKind::Type kind() const; HistoryKind::Type&
kind(); THistory& kind(HistoryKind kind);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::kind`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `kind() const`, `kind()` als Referenz und `kind(value)` mit
Rückgabe der Policy.

### §7.6.1 `THistory`: Zugriff auf `depth`

**Spec:** §7.6.1, S. 26 — "int32_t depth() const; int32_t& depth(); THistory&
depth(int32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::depth`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `depth() const`, `depth()` als Referenz und `depth(value)` mit
Rückgabe der Policy.

### §7.6.1 `THistory`: Getter als Referenz und Setter mit Rückgabe

**Spec:** §7.6.1, S. 26 — "HistoryKind::Type& kind(); THistory& kind(HistoryKind
kind); int32_t& depth(); THistory& depth(int32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Die Getter als Referenz (`kind()`, `depth()`) und die Setter mit
Rückgabe der Policy (verkettbar).

### §7.6.1 `THistory`: `KeepAll()` und `KeepLast(uint32_t)`

**Spec:** §7.6.1, S. 26 — "static History KeepAll(); static History
KeepLast(uint32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepAll`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepLast`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `History::KeepAll()` und `History::KeepLast(depth)`.

### §7.6.1 `History::KeepLast`: Typ und Vorgabe des Parameters

**Spec:** §7.6.1, S. 26 — "static History KeepLast(uint32_t depth);"

**Repo:** `crates/cpp/include/dds/core/policy/TCorePolicy.hpp::THistory::KeepLast`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_composition`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `KeepLast(uint32_t depth)`, wie in der Spec; die OMG-Header geben
keinen Vorgabewert an.

### §7.6.1 Alle Policies in `dds/qos/Policy.hpp`

**Spec:** §7.6.1, S. 26 — "The full set of policies is included in the mandatory
standard headers in the file dds/qos/Policy.hpp."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — Die PDF nennt `dds/qos/Policy.hpp`, die OMG-Header
`dds/core/policy/CorePolicy.hpp`; ZeroDDS folgt den Headern.

---

## §7.6.1 Policies von [DDS] 1.2

### §7.6.1 USER_DATA → `UserData`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 109 (`dds-1.2.pdf`) — "USER_DATA |
A sequence of octets: “value” | [...] The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::UserData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `UserData` mit den Attributen value; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 TOPIC_DATA → `TopicData`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) — "TOPIC_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TopicData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TopicData` mit den Attributen value; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 GROUP_DATA → `GroupData`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) — "GROUP_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::GroupData`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `GroupData` mit den Attributen value; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 DURABILITY → `Durability`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) — "DURABILITY
| A “kind”: VOLATILE, TRANSIENT_LOCAL, TRANSIENT, or PERSISTENT"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Durability`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Durability` mit den Attributen kind; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 DURABILITY_SERVICE → `DurabilityService`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 111 (`dds-1.2.pdf`) —
"DURABILITY_SERVICE | A duration "service_cleanup_delay", a HistoryQosPolicyKind
"history_kind", and three integers: history_depth, max_samples, max_instances,
max_samples_per_instance"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DurabilityService`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `DurabilityService` mit den Attributen service_cleanup_delay,
history_kind, history_depth, max_samples, max_instances, max_samples_per_instance; die
Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 PRESENTATION → `Presentation`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 111–112 (`dds-1.2.pdf`) —
"PRESENTATION | An “access_scope”: INSTANCE, TOPIC, GROUP And two booleans:
“coherent_access” “ordered_access”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Presentation`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Presentation` mit den Attributen access_scope, coherent_access,
ordered_access; die Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 DEADLINE → `Deadline`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 112 (`dds-1.2.pdf`) — "DEADLINE |
A duration “period”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Deadline`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Deadline` mit den Attributen period; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 LATENCY_BUDGET → `LatencyBudget`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) —
"LATENCY_BUDGET | A duration “duration” | [...] The default value of the
duration is zero"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::LatencyBudget`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `LatencyBudget` mit den Attributen duration; die Vorgabe der
Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 OWNERSHIP → `Ownership`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) — "OWNERSHIP |
A “kind” SHARED EXCLUSIVE"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Ownership`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Ownership` mit den Attributen kind; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 OWNERSHIP_STRENGTH → `OwnershipStrength`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) — "OWNERSHIP_
STRENGTH | An integer “value”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::OwnershipStrength`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `OwnershipStrength` mit den Attributen value; die Vorgabe der
Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 LIVELINESS → `Liveliness`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 114 (`dds-1.2.pdf`) — "LIVELINESS
| A “kind”: AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC and a duration
“lease_duration”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Liveliness`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Liveliness` mit den Attributen kind, lease_duration; die Vorgabe
der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 TIME_BASED_FILTER → `TimeBasedFilter`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 115 (`dds-1.2.pdf`) — "TIME_BASED_
FILTER | A duration "minimum_ separation" | [...] default minimum_separation=0"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TimeBasedFilter`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TimeBasedFilter` mit den Attributen minimum_separation; die
Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 PARTITION → `Partition`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 115 (`dds-1.2.pdf`) — "PARTITION |
A list of strings “name”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Partition`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Partition` mit den Attributen name; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 RELIABILITY → `Reliability`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 115–116 (`dds-1.2.pdf`) —
"RELIABILITY | A “kind”: RELIABLE, BEST_EFFORT and a duration
“max_blocking_time” | [...] BEST_EFFORT [...] This is the default value for
DataReaders and Topics. [...] RELIABLE [...] the default value for DataWriters.
[...] max_blocking_time=100ms."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Reliability`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `Reliability` mit den Attributen kind, max_blocking_time; die
Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 TRANSPORT_PRIORITY → `TransportPriority`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) — "TRANSPORT_
PRIORITY | An integer “value”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::TransportPriority`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `TransportPriority` mit den Attributen value; die Vorgabe der
Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 LIFESPAN → `Lifespan`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) — "LIFESPAN |
A duration “duration”"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::Lifespan`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `Lifespan` mit den Attributen duration; die Vorgabe der Tabelle von
DDS 1.2 und die Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 DESTINATION_ORDER → `DestinationOrder`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) —
"DESTINATION_ ORDER | A “kind”: BY_ RECEPTION_ TIMESTAMP, BY_SOURCE_ TIMESTAMP"

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DestinationOrder`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `DestinationOrder` mit den Attributen kind; die Vorgabe der Tabelle
von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 HISTORY → `History`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 117–118 (`dds-1.2.pdf`) — "HISTORY
| A “kind”: KEEP_LAST, KEEP_ALL And an optional integer “depth” | [...]
KEEP_LAST is the default kind. The default value of depth is 1."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::History`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `History` mit den Attributen kind, depth; die Vorgabe der Tabelle
von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 RESOURCE_LIMITS → `ResourceLimits`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 118–119 (`dds-1.2.pdf`) —
"RESOURCE_ LIMITS | Three integers: max_samples, max_instances,
max_samples_per_instance | [...] By default, LENGTH_UNLIMITED."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::ResourceLimits`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_default_qos_matches_the_specification`

**Status:** `done` — `ResourceLimits` mit den Attributen max_samples, max_instances,
max_samples_per_instance; die Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 ENTITY_FACTORY → `EntityFactory`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 119 (`dds-1.2.pdf`) — "ENTITY_
FACTORY | A boolean: “autoenable_ created_ entities” | [...] By default, TRUE."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::EntityFactory`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `EntityFactory` mit den Attributen autoenable_created_entities; die
Vorgabe der Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 WRITER_DATA_LIFECYCLE → `WriterDataLifecycle`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 119–120 (`dds-1.2.pdf`) —
"WRITER_DATA_ LIFECYCLE | A boolean: “autodispose_ unregistered_ instances” |
[...] By default, TRUE."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::WriterDataLifecycle`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `WriterDataLifecycle` mit den Attributen
autodispose_unregistered_instances; die Vorgabe der Tabelle von DDS 1.2 und die
Attribute prüft `test_every_policy_has_its_default_and_its_attributes`.

### §7.6.1 READER_DATA_LIFECYCLE → `ReaderDataLifecycle`

**Spec:** §7.6.1, S. 25; [DDS] 1.2 §7.1.3, S. 120 (`dds-1.2.pdf`) —
"READER_DATA_ LIFECYCLE | Two durations “autopurge_ nowriter_ samples_delay” and
“autopurge_ disposed_ samples_delay” | [...] By default, infinite."

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::ReaderDataLifecycle`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_every_policy_has_its_default_and_its_attributes`

**Status:** `done` — `ReaderDataLifecycle` mit den Attributen
autopurge_nowriter_samples_delay, autopurge_disposed_samples_delay; die Vorgabe der
Tabelle von DDS 1.2 und die Attribute prüft
`test_every_policy_has_its_default_and_its_attributes`.

---

## §7.6.2 Entity Class

### §7.6.2 Entity als Wurzel aller Entitäten

**Spec:** §7.6.2, S. 26 — "The Entity class is the root for all DDS entities, as
specified in the DDS v1.2 specification."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp`,
`crates/cpp/include/dds/core/Entity.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_entity_operations`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_reference_types`

**Status:** `done` — `dds::core::Entity` (`TEntity<detail::Entity>`) ist die Wurzel;
Participant, Topic, Publisher, Subscriber, DataWriter, DataReader haben `enable`,
`status_changes`, `instance_handle`, `close`, `retain`.

### §7.6.2 Freigabe bei null Referenzen oder durch `close`

**Spec:** §7.6.2, S. 26 — "Since an Entity is a reference type, its resources
are automatically managed by the middleware. Specifically, the resources
associated with the entity will be reclaimed either when the number of live
reference from the user application to the entity drops to zero, or when the
user explicitly invokes the method close."

**Repo:** `crates/cpp/include/dds/core/TEntity.hpp::TEntity::close`,
`crates/cpp/include/dds/core/detail/Retain.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_entities_and_statuses_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_objects_in_use_stay_open`

**Status:** `done` — Die Ressourcen einer Entität werden freigegeben, wenn die letzte
Referenz verschwindet (und kein Listener gesetzt und kein `retain()` aktiv ist) oder
`close()` aufgerufen wird.

---

## §7.6.2.1 QoS and Profiles

### §7.6.2.1 QosProvider lädt QoS über eine URI

**Spec:** §7.6.2.1, S. 26 — "This specification introduces the concept of a
QosProvider to load a QoS configuration from an URI. The URI is used to deduce
both the protocol to be used to access the QoS configuration as well as the
format in which it is expressed."

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp`,
`crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — Aus der URI folgen Protokoll und Format: `file:`, `data:` (RFC
2397), `http:`, `https:`; das Format ist DDS-XML
(`crates/zerodds-c-api/src/qos_provider_ffi.rs`, commits `5061c168b7f`, `9629b1b5a34`).

### §7.6.2.1 Mindestens URIs `file` und XML nach [DDS-CCM]

**Spec:** §7.6.2.1, S. 26 — "Implementation of this specification shall support
at very least file URIs and XML format compliant with the QoS-Profile defined in
the DDS for Lightweight CCM specification [DDS-CCM]."

**Repo:** `crates/cpp/include/dds/core/detail/QosProviderDelegate.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::crates/zerodds-c-api/tests/qos_provider.rs::a_provider_answers_the_qos_of_every_entity_kind_with_inheritance`

**Status:** `done` — `file:`-URIs mit XML-QoS-Profilen werden gelesen, mit Vererbung
(`base_name`); das Dokument folgt DDS-XML 1.0 (Elemente `domain_participant_qos`,
`*_QOS`-Namen der Aufzählungen, `<element>` in Partitionen).

### §7.6.2.1 `TQosProvider<DELEGATE>` in `dds::core::qos`

**Spec:** §7.6.2.1, S. 27 — "template <typename DELEGATE> class
dds::core::qos::TQosProvider : public dds::core::Reference<DELEGATE> {"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider`,
`crates/cpp/include/dds/core/QosProvider.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`

**Status:** `done` — `TQosProvider<DELEGATE> : Reference<DELEGATE>`; die PDF nennt den
Namensraum `dds::core::qos`, die OMG-Header `dds::core`; ZeroDDS folgt den Headern.

### §7.6.2.1 Konstruktoren mit URI und Profil

**Spec:** §7.6.2.1, S. 27 — "explicit TQosProvider(const std::string& uri, const
std::string& profile); explicit TQosProvider(const std::string& uri);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::TQosProvider`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`

**Status:** `done` — `QosProvider(uri, profile)` und `QosProvider(uri)`; ein Profil, das
das Dokument nicht hat, ist ein Fehler bei der Erzeugung.

### §7.6.2.1 `participant_qos()` und `participant_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::domain::qos::DomainParticipantQos
participant_qos(); dds::domain::qos::DomainParticipantQos participant_qos(const
std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::participant_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `participant_qos()` (Vorgabeprofil) und `participant_qos(id)`
(Profil `Bibliothek::Profil`) geben `DomainParticipantQos` zurück.

### §7.6.2.1 `topic_qos()` und `topic_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::topic::qos::TopicQos topic_qos();
dds::topic::qos::TopicQos topic_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::topic_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `topic_qos()` (Vorgabeprofil) und `topic_qos(id)` (Profil
`Bibliothek::Profil`) geben `TopicQos` zurück.

### §7.6.2.1 `subscriber_qos()` und `subscriber_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::sub::qos::SubscriberQos subscriber_qos();
dds::sub::qos::SubscriberQos subscriber_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::subscriber_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `subscriber_qos()` (Vorgabeprofil) und `subscriber_qos(id)` (Profil
`Bibliothek::Profil`) geben `SubscriberQos` zurück.

### §7.6.2.1 `datareader_qos()` und `datareader_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::sub::qos::DataReaderQos datareader_qos();
dds::sub::qos::DataReaderQos datareader_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::datareader_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `datareader_qos()` (Vorgabeprofil) und `datareader_qos(id)` (Profil
`Bibliothek::Profil`) geben `DataReaderQos` zurück.

### §7.6.2.1 `publisher_qos()` und `publisher_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::pub::qos::PublisherQos publisher_qos();
dds::pub::qos::PublisherQos publisher_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::publisher_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `publisher_qos()` (Vorgabeprofil) und `publisher_qos(id)` (Profil
`Bibliothek::Profil`) geben `PublisherQos` zurück.

### §7.6.2.1 `datawriter_qos()` und `datawriter_qos(id)`

**Spec:** §7.6.2.1, S. 27 — "dds::pub::qos::DataWriterQos datawriter_qos();
dds::pub::qos::DataWriterQos datawriter_qos(const std::string& id);"

**Repo:** `crates/cpp/include/dds/core/TQosProvider.hpp::TQosProvider::datawriter_qos`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_qos_provider`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_sample_info_parts_qos_provider_kinds_and_filter_expression`

**Status:** `done` — `datawriter_qos()` (Vorgabeprofil) und `datawriter_qos(id)` (Profil
`Bibliothek::Profil`) geben `DataWriterQos` zurück.

### §7.6.2.1 Beispiel für QosProvider

**Spec:** §7.6.2.1, S. 27–28 — "Below a non mandatory example showing how the
QosProvider can be used: dds::core::qos::QosProvider
qos_provider("file:///smwr/hdd/config-qos.xml", "myprofile");
DataReader<ShapeType> dr(sub, topic, qos_provider.datareader_qos());"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel der Spec; `test_qos_provider` macht daraus
einen Writer mit der QoS des Providers.

---

## §7.7 Domain Package

### §7.7 Klassen des Pakets domain

**Spec:** §7.7, S. 28 — "The domain package defines the
DomainParticipantFactory, DomainParticipant, and DomainParticipantListener."

**Repo:** `crates/cpp/include/dds/domain/DomainParticipant.hpp`,
`crates/cpp/include/dds/domain/DomainParticipantListener.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_domain_participant_listener`

**Status:** `done` — `DomainParticipant` und `DomainParticipantListener` stehen in
`dds::domain`. Eine `DomainParticipantFactory` haben die OMG-Header nicht (siehe
„DomainParticipantFactory: get_instance“); nach §2 gehen die Header der PDF vor.

### §7.7 Vollständige Angabe in den Headern

**Spec:** §7.7, S. 28 — "For a complete reference see the standard header
files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf die Header; sie sind mit den OMG-Headern
verglichen (siehe §2).

---

## §7.8 Topic Package

### §7.8 Klassen des Pakets topic

**Spec:** §7.8, S. 28 — "The topic packaged defines the classes related to topic
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

**Status:** `done` — `Topic`, `TopicDescription`, `ContentFilteredTopic`, `MultiTopic` und `TopicListener` stehen in `dds::topic`.

### §7.8 Topic über den Typ parametrisiert, registriert den Typ selbst

**Spec:** §7.8, S. 28 — "The topic class is parameterized in the topic type and
transparently performs the registration of type support."

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp::Topic`,
`crates/cpp/include/dds/topic/TopicTraits.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_topic_any_topic_and_listener`

**Status:** `done` — `Topic<T>` registriert den Typ beim Erzeugen über
`topic_type_support<T>`.

### §7.8 Topic anlegen

**Spec:** §7.8, S. 28 — "DomainParticipant dp(domainId);
dds::topic::Topic<RadarTrack> topic(dp, "RadarTrackTopic");"

**Repo:** `crates/cpp/include/dds/topic/Topic.hpp::Topic`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`

**Status:** `done` — `DomainParticipant dp(domainId); Topic<RadarTrack> topic(dp,
"RadarTrackTopic");` übersetzt und läuft (`generated_psm_roundtrip.cpp` mit dem Typ
`RadarTrack` aus §8.2).

### §7.8 Topic mit QoS anlegen

**Spec:** §7.8, S. 28 — "dds::qos::TopicQos tqos = dp.default_topic_qos(); tqos
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
übersetzt und wirkt. Die PDF schreibt `dds::qos::TopicQos`, die OMG-Header
`dds::topic::qos::TopicQos`.

---

## §7.9 Pub Package

### §7.9 Klassen des Pakets pub

**Spec:** §7.9, S. 28 — "The publication (pub) package defines all the classes
associated with the production of data. As such, it defines the Publisher, the
DataWriter and their associated listeners as well as any types."

**Repo:** `crates/cpp/include/dds/pub/Publisher.hpp`,
`crates/cpp/include/dds/pub/DataWriter.hpp`,
`crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/pub/PublisherListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `Publisher`, `DataWriter`, `AnyDataWriter` und ihre Listener stehen
in `dds::pub`.

### §7.9 Verbindliche Klassen in den Headern

**Spec:** §7.9, S. 28 — "The mandatory classes are specified in the standard
header files. Below, we focus on the specifics of the DataWriter class."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf die Header (siehe §2).

### §7.9.1 DataWriter über Delegate und Typ parametrisiert

**Spec:** §7.9.1, S. 28 — "The DataWriter class is parameterized with respect to
the delegate and the topic type that it writes."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`

**Status:** `done` — `DataWriter<T, DELEGATE>` hat wie die OMG-Header `template <typename T, template <typename Q> class DELEGATE = detail::DataWriterDelegate>` (und `DataReader<T, DELEGATE>` ebenso); `DataWriter<T>` ist `DataWriter<T, detail::DataWriterDelegate>` und `DELEGATE_T` ist `DELEGATE<T>` (`static_assert` in `test_reference_types`).

### §7.9.1 Überladene Operationen `write` für einzelne Samples

**Spec:** §7.9.1, S. 28 — "The class provides several different overloaded
methods for writing data by providing single samples"

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(sample)`, `write(sample, time)`, `write(sample, handle[,
time])`, `write(TopicInstance[, time])` und `operator<<` (Sample, Sample mit Zeit,
Sample mit Handle).

### §7.9.1 Schreiben über Iteratoren

**Spec:** §7.9.1, S. 28 — "The class provides several different overloaded
methods for writing data by providing single samples or iterators over samples."

**Repo:** `crates/cpp/include/dds/pub/DataWriter.hpp::DataWriter::write`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_instance_operations_with_a_time_stamp`

**Status:** `done` — `write(begin, end[, time])` und `write(samples_begin, samples_end,
handles_begin, handles_end[, time])`.

---

## §7.10 Sub Package

### §7.10 Klassen des Pakets sub

**Spec:** §7.10, S. 28 — "The subscription (sub) package defines all the classes
associated with the consumption of data. As such, it defines the Subscriber, the
DataReader and their associated listeners as well as any types."

**Repo:** `crates/cpp/include/dds/sub/Subscriber.hpp`,
`crates/cpp/include/dds/sub/DataReader.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp`,
`crates/cpp/include/dds/sub/SubscriberListener.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_datawriter_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `Subscriber`, `DataReader`, `AnyDataReader` und ihre Listener
stehen in `dds::sub`.

### §7.10 Verbindliche Klassen in den Headern

**Spec:** §7.10, S. 28 — "The mandatory classes are specified in the standard
header files. Below, we focus on the specifics of the DataReader class."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf die Header (siehe §2).

---

## §7.11 Extensible and Dynamic Type Support Package

### §7.11 Annotationen und dynamische Typen

**Spec:** §7.11, S. 29 — "The Extensible and Dynamic Type Support (xtypes)
package defines all the classes associated with the definition of extensible
topics, such as annotations and the definition and manipulation of dynamic
types."

**Repo:** `crates/cpp/include/dds/core/xtypes/Annotations.hpp`,
`crates/cpp/include/dds/core/xtypes/TAnnotation.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_dynamic_types_dynamic_data_and_type_provider`

**Status:** `done` — Die Annotationen der OMG-Header: `Key`, `Optional`, `Shared`,
`Nested`, `Id`, `Final`, `Extensible`, `Mutable`, `Extensibility`, `MustUnderstand`,
`Verbatim`, `Bitset`, `BitBound`; ihre Art und ihre Werte prüft der Test.

### §7.11 Beschreiben, Erzeugen und Annotieren dynamischer Typen

**Spec:** §7.11, S. 29 — "As such, this package introduces all classes necessary
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

**Status:** `done` — Typen werden aus Klassen gebaut (primitive, String, Sequenz, Map, Array, Struktur, Union, Enumeration, Alias, Bitset, `BitMaskType`), annotiert (`add_annotation`) und geändert (`add_member`, `remove_member`); eine Union hat einen `default`-Zweig (`UnionCase(member)`, `is_default()`) und kann auf eine Enumeration schalten (`UnionType<uint32_t>(name, EnumerationType, cases)`); ein Bitmask-Wert in `DynamicData` ist die Menge der Namen der gesetzten Flags. `DynamicData` liest und schreibt Member nach Namen und Id, kodiert und dekodiert (XCDR1/2, Little/Big Endian) und geht durch JSON; `TypeProvider` lädt Typen aus IDL oder einer DDS-XML-Typbibliothek, auch Bitmask, Union mit `default` und Union auf Enumeration; `Topic<DynamicData>` trägt Samples zwischen Writer und Reader und beantwortet eine Abfrage auf einem Feld. `fixed` hat im XTypes-Typsystem keinen Wert (`TypeKind` hat keinen, das C-API meldet `Unrepresentable`, DDSXTY14-28); eine Struktur mit `fixed`-Member wird nicht als DynamicType geladen. `DynamicData::value<T>` nimmt keine Folge und kein Array; sie gehen über `from_json_text`. `DataRepresentation` und `TypeConsistencyEnforcement` stehen wie in den OMG-Headern in keinem QoS-Satz.

---

## §7.12 C++11 Compatibility

### §7.12 Nur C++03, Unterstützung einzelner Merkmale von C++11

**Spec:** §7.12, S. 29 — "This specification relies on C++03 features only.
However, to improve its efficiency and usability in a C++11 environment, it
provides built-in support for some C++11 features, such as initializer lists."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §7.12; die einzelnen Merkmale haben je
ein Item. Die Header übersetzen als C++03 (siehe „§3 [C++] ISO/IEC 14882:2003“), die Merkmale von C++11 stehen hinter `ZERODDS_CXX11`.

### §7.12 Funktion `move(LoanedSamples<T>&)`

**Spec:** §7.12, S. 29 — "• A move(LoanedSamples<T>&) function shall be defined
in the same namespace as LoanedSamples<T> that behaves identical to std::move."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::move`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::sub::move(LoanedSamples<T>&)` gibt die Samples verschoben
zurück; der Test prüft, dass die Quelle danach leer ist.

### §7.12 `cbegin()` und `cend()` von LoanedSamples und SharedSamples

**Spec:** §7.12, S. 29 — "• LoanedSamples<T> and SharedSamples<T> shall provide
member cbegin() and cend() functions, which return const_iterator irrespective
of the constness of the object."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::LoanedSamples::cbegin`,
`crates/cpp/include/dds/sub/SharedSamples.hpp::SharedSamples::cbegin`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `cbegin()` und `cend()` als Members und als Funktionen im
Namensraum; sie geben `const_iterator`.

### §7.12 C++11: LoanedSamples nur verschiebbar

**Spec:** §7.12, S. 29 — "• LoanedSamples<T> shall be implemented as a
first-class move-only type using move operations. A representative example is
std::uniqe_ptr."

**Repo:** `crates/cpp/include/dds/sub/LoanedSamples.hpp::LoanedSamples`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — Kopieren ist gelöscht, Verschieben vorhanden (`static_assert` im
Test).

### §7.12 C++11: `begin()` und `end()` im Namensraum

**Spec:** §7.12, S. 29 — "• LoanedSamples<T> and SharedSamples<T> shall provide
namespace level begin() and end() functions to facilitate use of range-based for
loop."

**Repo:** `crates/cpp/include/dds/core/array.hpp::begin`,
`crates/cpp/include/dds/sub/LoanedSamples.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_samples_api`

**Status:** `done` — `dds::core::begin`/`end` und die Members für die bereichsbasierte
Schleife; der Test benutzt sie für `SharedSamples` und `LoanedSamples`.

### §7.12 `dds::core::array` als Alias von `std::array`

**Spec:** §7.12, S. 29 — "• dds::core::array shall be a template typedef to
std::array."

**Repo:** `crates/cpp/include/dds/core/array.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_layout_headers_and_find`

**Status:** `done` — `template <typename T, std::size_t N> using array = std::array<T,
N>;`.

### §7.12 C++11: Aufzählungen mit `enum class`

**Spec:** §7.12, S. 29 — "• Enumerations shall use built-in type-safe
enumerations with enum class syntax."

**Repo:** `crates/cpp/include/dds/core/SafeEnumeration.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — Die erzeugten Aufzählungen sind `enum class`
(`an_enumeration_takes_an_underlying_type_from_its_bit_bound` in
`crates/lang-rules/src/adapter/cpp.rs`); die Arten der Policies sind `safe_enum` nach
§7.4.3.

### §7.12 C++11: Verschieben für alle Werttypen

**Spec:** §7.12, S. 29 — "• Move operations (move constructor and move assign)
shall be provided for all Value<DELEGATE> types."

**Repo:** `crates/cpp/include/dds/core/Value.hpp`,
`crates/cpp/include/dds/core/policy/TCorePolicy.hpp`,
`crates/cpp/include/dds/core/status/TStatus.hpp`,
`crates/cpp/include/dds/core/TEntityQos.hpp`

**Tests:**
`crates/cpp/tests/smoke_dds_psm.cpp::test_value_and_reference_classes_without_other_coverage`

**Status:** `done` — `Value<D>` und die Werttypen (Policies, QoS-Mengen, Samples,
`SampleInfo`, Status) haben Verschiebekonstruktor und -zuweisung; der Test prüft, dass
Daten übergehen und die Quelle leer bleibt.

### §7.12 C++11: erzeugte Typen mit Verschieben nach idl2cpp11

**Spec:** §7.12, S. 29 — "• Plain language binding shall be augmented as follows
• Generated code for complex types shall use move operations (move-assignment,
move-constructor) as defined in idl2cpp11 (ptc/2012-04-03) struct type mapping."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die erzeugten Klassen (Vorgabe, `--cpp-mapping psm`) haben die implizit
erzeugten Verschiebeoperationen (`static_assert` und Verschieben im Roundtrip).
idl2cpp11 (ptc/2012-04-03) liegt nicht im Cache.

### §7.12 C++11: Arrays als `const`-Referenz

**Spec:** §7.12, S. 29 — "• Structures containing arrays shall use a
const-reference parameter for arrays as opposed to pass-by-value."

**Repo:** `tools/idlc/src/generate.rs`, `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:**
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — Der Konstruktor der erzeugten Klasse nimmt ein Array als `const
::dds::core::array<…>&` (Rust-Test des Codegen).

### §7.12 C++11: `swap` im Namensraum und als Member

**Spec:** §7.12, S. 29 — "• A namespace level swap(t1) and a member swap shall
be provided for each generated class."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`,
`crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_structure_has_swap_as_a_member_and_in_its_namespace`

**Status:** `done` — Jede erzeugte Klasse hat `swap` als Member und im Namensraum
(Roundtrip und Rust-Test des Codegen).

### §7.12 C++11: `noexcept` erlaubt

**Spec:** §7.12, S. 29 — "• Move-assign, move-constructor, and member swap
functions, and namespace-level swap may provide noexcept specification to allow
efficient and exception-safe resizing of standard containers."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; keine Anforderung.

---

## §7.13 Examples

### §7.13.1 Beispiel nach C++03

**Spec:** §7.13.1, S. 29–31 — "This sub clause provides an example for full
application writing and reading RadarTracks topics."

**Repo:** `crates/cpp/tests/generated_psm_roundtrip.cpp`,
`crates/cpp/tests/generated/psm_probe.idl`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Anwendung aus §7.13.1 (Partition, Reliable, Transient,
KeepLast(10), TransportPriority(14), `DataWriterQos dwqos(tqos)`, `write`, `<<`, Lesen
in einen Vektor von Samples) läuft über den Typ `RadarTrack` aus §8.2. Abweichungen der
Spec von sich selbst: das Beispiel schreibt `Samples<RadarTrack>` für
`Sample<RadarTrack>`, `drqos` ist nicht deklariert und `RadarTrack track("alpha", 100,
200)` hat drei Argumente gegen fünf in §8.2.

### §7.13.2 Beispiel nach C++11

**Spec:** §7.13.2, S. 31–32 — "While not requiring C++11 the DDS-PSM-Cxx API
described in this specification has built-in support for some of the most
interesting C++11 features."

**Repo:** `crates/cpp/tests/generated_psm_roundtrip.cpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — Die Auswahl
`dr.select().max_samples(100).state(DataState::…).take()` mit bereichsbasierter Schleife
läuft. Die PDF schreibt `.data(DataState::new_data())` und `samples.data()` in der
Schleife; die OMG-Header haben `state(…)`.

---

## §8.1 Type Mapping

### §8.1 Typsystem nach [DDS-XTypes], Rest nach dessen Binding

**Spec:** §8.1, S. 33 — "The type system for DDS topic types is defined by the
Extensible and Dynamic Topic Types for DDS specification [DDS-XTypes]. This sub
clause defines the set of rules to be used in order to map abstract DDS topic
types into C++ types that can be used by application programmers. Those aspects
of the DDS Type System that are not addressed below are as specified in the
Plain Language Binding as defined by [DDS- XTypes] (which in turn is defined in
terms of an IDL-to-C++ mapping)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §8.1; die Regeln haben je ein Item.

### §8.1.1 Aggregattypen als Klasse

**Spec:** §8.1.1, S. 33 — "DDS aggregation types shall be mapped to a C++
class."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`, `tools/idlc/src/generate.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** `done` — In der Vorgabe (`--cpp-mapping psm`) ist jede Struktur eine Klasse (`class
Reading`) mit privaten Mitgliedern und Zugriffsfunktionen.

### §8.1.1 Attribute gekapselt

**Spec:** §8.1.1, S. 33 — "Contained attributes shall be encapsulated."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — Die Attribute sind private Mitglieder (`id_`, `value_`, …).

### §8.1.1 Zugriffsfunktionen nach §7.4

**Spec:** §8.1.1, S. 33 — "Accessors shall be provided following the rules
described in 7.4."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — `NT attribute() const`, `NT& attribute()`, `attribute(value)` für
einfache Typen; `const CT&`, `CT&` und Setter für zusammengesetzte Typen (siehe die
Items zu §7.4.6).

### §8.1.1 Innerer Zustand nicht festgelegt

**Spec:** §8.1.1, S. 33 — "The representation of internal state is unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage der Spec; keine Anforderung.

### §8.1.2 Einfache Typen und Folgen nach Table 7.1

**Spec:** §8.1.2, S. 33 — "IDL primitive and collection types used to define a
topic type shall be mapped to C++ following the rules listed in Table 7.1."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — Alle Zeilen von Table 7.1 in der Vorgabe (`--cpp-mapping psm`; `static_assert`
je Zeile); die Option `--cpp-mapping native` weicht bei `map` und `long double` ab (siehe die Items
zu Table 7.1).

### §8.1.3 Aufzählungen mit gleichem Namen und gleichen Konstanten

**Spec:** §8.1.3, S. 33 — "IDL enumerations shall be mapped into C++
enumerations with exactly the same enumeration name and enumeration constants."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`

**Status:** `done` — `enum class Colour { RED, GREEN, BLUE }` mit dem Namen und den
Konstanten der IDL; der Roundtrip benutzt `Colour::GREEN`.

### §8.1.4 `@Optional` → `dds::core::optional<T>`

**Spec:** §8.1.4, S. 33 — "Attributes annotated though the @Optional annotation
are mapped to a template instantiation of the class dds::core::optional<T> with
T equal to the type attribute would normally map as per the rules specified
above."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — Ein `@optional`-Attribut ist `dds::core::optional<T>` (`maybe()`,
`z()` in `RadarTrack`); die Anwesenheit geht durch den Codec.

### §8.1.4 `dds::core::optional` und `@Optional`

**Spec:** §8.1.4, S. 33 — "mapped to a template instantiation of the class
dds::core::optional<T>"

**Repo:** `crates/cpp/include/dds/core/Optional.hpp`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`test_layout_headers_and_find`

**Status:** `done` — `dds::core::optional<T>` mit `is_set()`, `get()`, `set(v)`,
`reset()`.

### §8.1.5 `@Shared` → Zeiger

**Spec:** §8.1.5, S. 33 — "Attributes annotated through the @Shared annotation
are mapped to a pointer of the type they would normally map as per the rules
specified above."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs`,
`crates/idl-lower/src/annotation.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — Ein Attribut mit `@Shared` (XTypes 1.0 Beta 1, in XTypes 1.3
`@external`) ist `std::shared_ptr<T>` (`plot()` in `RadarTrack`); `@Shared` wird zu
`@external` (`idl-lower`). Das Beispiel der PDF benutzt `std::vector<uint8_t>*` im
Konstruktor und `plot_ref_t` im Zugriff; das ist untereinander verschieden.

### §8.1.5 `@Shared`

**Spec:** §8.1.5, S. 33 — "Attributes annotated through the @Shared annotation"

**Repo:** `crates/idl-lower/src/annotation.rs`

**Tests:** `crates/cpp/tests/generated_psm_roundtrip.cpp::main`,
`tools/idlc/src/generate.rs::tests::the_psm_mapping_spells_the_types_of_table_7_1`

**Status:** `done` — Der Name `@Shared` (ohne Beachtung der Groß- und Kleinschreibung)
ist als Annotation bekannt und wird nicht als unbekannte Annotation übergangen.

---

## §8.2 Example

### §8.2 Beispiel `RadarTrack`

**Spec:** §8.2, S. 33–34 — "This sub clause provides a simple yet representative
example demonstrating the ISO/IEC mapping for DDS types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel der Spec; `psm_probe.idl` enthält den Typ
`RadarTrack`, `generated_psm_roundtrip.cpp` benutzt ihn.

---

## Audit-Status

396 done / 0 partial / 0 open / 54 n/a (informative) / 0 n/a (rejected).

Test-Lauf: die Ergebnisse der zitierten Tests kommen vom Linux-Testhost.

