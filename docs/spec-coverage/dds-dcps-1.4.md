# DDS DCPS 1.4 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-dcps-1.4.pdf` (180 Seiten, OMG formal/2015-04-10) —
[OMG DDS 1.4](https://www.omg.org/spec/DDS/1.4/)

Item-für-Item-Audit gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 12). Jede
Operation, jede Rückgabe- und Vorbedingungsregel, jedes QoS-Feld, jedes
Status-Feld und jedes Feld eines Built-in-Topics ist ein eigenes Item. `done`
steht nur, wo Code und Test die Anforderung abdecken; ein Test-Zitat nennt die
Testfunktion als `datei::modul::name`.

**Kontext:** Die DCPS-Seite von ZeroDDS ist verteilt über:

- `crates/dcps/` — die Rust-API (`DomainParticipantFactory`,
  `DomainParticipant`, `Topic`, `ContentFilteredTopic`, `MultiTopic`,
  `Publisher`, `DataWriter`, `Subscriber`, `DataReader`, Conditions, `WaitSet`,
  Listener, Status, `SampleInfo`, Built-in-Topics) und die Laufzeit
  (`runtime.rs`), die sie an RTPS bindet.
- `crates/qos/` — die QoS-Policies mit Defaults, Kompatibilitätsregeln und
  Konsistenzprüfungen.
- `crates/sql-filter/` — die Filter- und Abfragesprache von Annex B.
- `crates/zerodds-c-api/` — die C-Bindung. Sie ist hier nur dort zitiert, wo
  die Rust-API eine PIM-Operation nicht anbietet; ihr übriges Verhalten ist
  nicht Gegenstand dieses Audits.

Die Rust-API ist keine Abbildung der IDL-PSM von §2.3: Rückgabecodes sind
`Result<T, DdsError>`, Sammlungen sind `Vec`, ein Typ wird als Typparameter
`T: DdsType` übergeben. Wo die Rust-Signatur einen Parameter des PIM nicht hat
(etwa Listener und Maske bei den `create_*`-Operationen), ist das als
Abweichung geführt.

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Overview

### §1.1 Introduction

**Spec:** §1.1, S. 13 — "This specification defines both the Application
Interfaces (APIs) and the Communication Semantics (behavior and quality of
service) that enable the efficient delivery of information from information
producers to matching consumers." Es folgen Entwurfsziele der Schnittstellen
(Vorab-Allokation, keine unbegrenzten Ressourcen, wenige Kopien, typisierte
Schnittstellen, QoS als Liste unabhängiger Policies, Trennung von Publikations-
und Abonnementseite).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck und Entwurfsziele der Spec; sie richten
sich an den Entwurf der Schnittstellen, nicht an eine Implementierung. Die
Anforderungen stehen in §2.2 und §2.3 und sind dort einzeln geführt.

### §1.2 Purpose

**Spec:** §1.2, S. 13–14 — Einsatzgebiete, das Modell des „global data space“,
Verweis auf DDSI-RTPS für die Interoperabilität auf der Leitung.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund und Abgrenzung; keine Anforderung.
Die Leitungsseite ist in `ddsi-rtps-2.5.md` geführt.

---

## §2.1 Summary

### §2.1 Summary

**Spec:** §2.1, S. 15 — "The description is organized into two sub clauses: The
Platform Independent Model (PIM). The Platform Specific Model (PSM) for the OMG
IDL platform based on the PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung der Spec.

---

## §2.2.1 Overview and Design Rationale

### §2.2.1.1 Format and Conventions — Klassentabellen

**Spec:** §2.2.1.1, S. 15–16 — Form der Klassentabellen; Parameter ohne
Modifikator sind „in“; "<type> [ ]" bezeichnet eine Sammlung, deren Abbildung
die PSM festlegt.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notationsregel der Spec.

### §2.2.1.1 ReturnCode_t — Abbildung durch die PSM

**Spec:** §2.2.1.1, S. 16 — "At the PIM level we have modeled errors as
operation return codes typed ReturnCode_t. Each PSM may map these to either
return codes or exceptions."

**Repo:** `crates/dcps/src/error.rs` (`DdsError`, `Result`,
`DdsError::as_return_code`, Modul `return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`,
`crates/dcps/src/error.rs::tests::rc_error_maps_for_wire_and_transport_and_other`

**Status:** done — Fehler sind `Err(DdsError)`, Erfolg ist `Ok`;
`as_return_code` liefert den Zahlenwert des IDL-PSM.

### §2.2.1.1 ReturnCode_t OK

**Spec:** §2.2.1.1, S. 17 — "OK Successful return."

**Repo:** `crates/dcps/src/error.rs` (`Result::Ok`, `return_code::OK`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`

**Status:** done

### §2.2.1.1 ReturnCode_t ERROR

**Spec:** §2.2.1.1, S. 17 — "ERROR Generic, unspecified error."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Other`, `DdsError::WireError`,
`DdsError::TransportError` → `return_code::ERROR`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_error_maps_for_wire_and_transport_and_other`

**Status:** done

### §2.2.1.1 ReturnCode_t BAD_PARAMETER

**Spec:** §2.2.1.1, S. 17 — "BAD_PARAMETER Illegal parameter value."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::BadParameter`,
`DdsError::Configuration` → `return_code::BAD_PARAMETER`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_bad_parameter_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t UNSUPPORTED

**Spec:** §2.2.1.1, S. 17 — "UNSUPPORTED Unsupported operation. Can only be
returned by operations that are optional."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Unsupported` →
`return_code::UNSUPPORTED`); zurückgegeben u. a. von
`crates/dcps/src/publisher.rs` (`DataWriter::get_key_value`) und
`crates/dcps/src/subscriber.rs` (`DataReader::get_key_value`,
`DataReader::decode_or_keyholder`).

**Tests:** `crates/dcps/src/error.rs::tests::rc_unsupported_maps`,
`crates/dcps/tests/unsupported_qos.rs::a_data_representation_the_service_does_not_emit_is_unsupported`

**Status:** done — der Code ist abgebildet, und eine Operation liefert ihn nur für einen Wert, den der Dienst nicht unterstützt (DATA_REPRESENTATION ausser XCDR1, XCDR2 und XML). `get_key_value` liefert `Unsupported` nicht mehr (§2.2.2.4.2.9, §2.2.2.5.3.29); die übrigen Quellen von `DdsError::Unsupported` sind die optionale Flat-Data-Anbindung und ein Fehler des Betriebssystems.

### §2.2.1.1 ReturnCode_t ALREADY_DELETED

**Spec:** §2.2.1.1, S. 17 — "ALREADY_DELETED The object target of this
operation has already been deleted."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::AlreadyDeleted`);
`crates/dcps/src/entity.rs` (`EntityState::mark_deleted`,
`EntityState::check_not_deleted`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_already_deleted_maps`,
`crates/dcps/src/entity.rs::tests::check_not_deleted_returns_already_deleted_after_mark`

**Status:** done — der Code ist abgebildet; wo er geliefert wird, steht unter
der Regel „Verwendung einer gelöschten Entity“ unten.

### §2.2.1.1 ReturnCode_t OUT_OF_RESOURCES

**Spec:** §2.2.1.1, S. 17 — "OUT_OF_RESOURCES Service ran out of the resources
needed to complete the operation."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::OutOfResources`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_out_of_resources_maps`,
`crates/dcps/src/condition.rs::tests::waitset_attach_above_max_returns_out_of_resources`

**Status:** done

### §2.2.1.1 ReturnCode_t NOT_ENABLED

**Spec:** §2.2.1.1, S. 17 — "NOT_ENABLED Operation invoked on an Entity that is
not yet enabled."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::NotEnabled`);
`crates/dcps/src/entity.rs` (`EntityState::check_enabled`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_not_enabled_maps`,
`crates/dcps/src/entity.rs::tests::check_enabled_returns_not_enabled_for_disabled_entity`

**Status:** done — der Code ist abgebildet; welche Operationen ihn liefern,
steht unter §2.2.2.1.1.7.

### §2.2.1.1 ReturnCode_t IMMUTABLE_POLICY

**Spec:** §2.2.1.1, S. 17 — "IMMUTABLE_POLICY Application attempted to modify
an immutable QosPolicy."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::ImmutablePolicy`);
`crates/dcps/src/entity.rs` (`immutable_if_enabled`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_immutable_policy_maps`,
`crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`

**Status:** done

### §2.2.1.1 ReturnCode_t INCONSISTENT_POLICY

**Spec:** §2.2.1.1, S. 17 — "INCONSISTENT_POLICY Application specified a set of
policies that are not consistent with each other."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::InconsistentPolicy`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_inconsistent_policy_maps`

**Status:** done — der Code ist abgebildet; dass die `create_*`- und
`set_qos`-Operationen ihn bei einer inkonsistenten QoS nicht liefern, ist unter
§2.2.2.1.1.1 und den `create_*`-Items geführt.

### §2.2.1.1 ReturnCode_t PRECONDITION_NOT_MET

**Spec:** §2.2.1.1, S. 17 — "PRECONDITION_NOT_MET A pre-condition for the
operation was not met."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::PreconditionNotMet`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_precondition_not_met_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t TIMEOUT

**Spec:** §2.2.1.1, S. 17 — "TIMEOUT The operation timed out."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Timeout`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_timeout_maps`,
`crates/dcps/src/condition.rs::tests::waitset_wait_timeout_returns_err`

**Status:** done

### §2.2.1.1 ReturnCode_t ILLEGAL_OPERATION

**Spec:** §2.2.1.1, S. 17 — "ILLEGAL_OPERATION An operation was invoked on an
inappropriate object or at an inappropriate time … There is no precondition
that could be changed to make the operation succeed."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::IllegalOperation`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_illegal_operation_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t NO_DATA

**Spec:** §2.2.1.1, S. 17 — "NO_DATA Indicates a transient situation where the
operation did not return any data but there is no inherent error."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::NoData`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_no_data_maps`

**Status:** done — der Code ist abgebildet; dass `read`/`take` ohne Daten
`Ok` mit leerer Liste statt NO_DATA liefern, ist unter §2.2.2.5.3.8 geführt.

### §2.2.1.1 Standard-Rückgabecodes

**Spec:** §2.2.1.1, S. 17 — "The return codes OK, ERROR, ILLEGAL_OPERATION,
ALREADY_DELETED, UNSUPPORTED, and BAD_PARAMETER are the standard return codes
and the specification won’t mention them explicitly for each operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Lesekonvention für die folgenden
Operationsbeschreibungen.

### §2.2.1.1 Verwendung einer gelöschten Entity

**Spec:** §2.2.1.1, S. 17 — "It is an error for an application to use an Entity
that has already been deleted … In the cases where the implementation can
detect the use of a deleted entity, the operation should fail and return
ALREADY_DELETED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_not_deleted`,
`EntityState::check_enabled`); aufgerufen in `crates/dcps/src/publisher.rs`
(`Publisher::create_datawriter`, DataWriter-Operationen),
`crates/dcps/src/subscriber.rs` (DataReader- und Subscriber-Operationen),
`crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`,
`DomainParticipant::delete_publisher`, `DomainParticipant::delete_subscriber`).

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`,
`crates/dcps/tests/entity_deletion.rs::participant_delete_contained_entities_deletes_everything_it_created`,
`crates/dcps/src/entity.rs::tests::check_not_deleted_returns_already_deleted_after_mark`

**Status:** done — Publisher, Subscriber, DataWriter, DataReader, Topic, ContentFilteredTopic und MultiTopic tragen den Zustand „gelöscht“ und liefern danach `AlreadyDeleted`. Der `DomainParticipant` ist nach `delete_participant` gelöscht. `create_publisher` und `create_subscriber` liefern keinen `Result` (die Spec lässt ein create-`nil` zurückkommen): sie liefern auf einem gelöschten Participant eine gelöschte Entity, `try_create_publisher` und `try_create_subscriber` den Fehler `AlreadyDeleted`.

### §2.2.1.2.1 Conceptual Outline — Overview

**Spec:** §2.2.1.2.1, S. 18–19 — Publisher/DataWriter auf der sendenden,
Subscriber/DataReader auf der empfangenden Seite; ein Topic verbindet einen
im Domain eindeutigen Namen, einen Datentyp und die QoS der Daten.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsübersicht; die Anforderungen an die
Klassen stehen in §2.2.2 und sind dort einzeln geführt.

### §2.2.1.2.2 Overall Conceptual Model

**Spec:** §2.2.1.2.2, S. 19–21 — jede Entity hat QoS, höchstens einen Listener
und eine StatusCondition; alle Entities hängen an einem DomainParticipant;
gleiche Schlüsselwerte bezeichnen dieselbe Instanz, ohne Schlüssel hat ein
Topic genau eine Instanz; Built-in-DataReader "should be provided with every
implementation" (Fußnote 8).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Aussagen sind normativ an anderer Stelle
gefasst und dort geführt: ein Listener je Entity unter §2.2.2.1.1.3, Instanzen
und Schlüssel unter §2.2.2.5.1, Built-in-Topics unter §2.2.5.

### §2.2.2 PIM Description — Module

**Spec:** §2.2.2, S. 21–22 — fünf Module: Infrastructure, Domain,
Topic-Definition, Publication, Subscription; Entities, die die Anwendung
erweitert, sind als Interface modelliert, die übrigen als Klassen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Module sind unter
§2.2.2.1–§2.2.2.5 einzeln geführt.

---

## §2.2.2.1 Infrastructure Module

### §2.2.2.1 Klassen des Infrastructure Module

**Spec:** §2.2.2.1, S. 23 — Entity, DomainEntity, QosPolicy, Listener, Status,
WaitSet, Condition, GuardCondition, StatusCondition.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; jede Klasse ist unten einzeln
geführt.

### §2.2.2.1.1 Entity — abstrakte Basis

**Spec:** §2.2.2.1.1, S. 23–24 — "This class is the abstract base class for all
the DCPS objects that support QoS policies, a listener and a status condition."
Operationen: set_qos, get_qos, set_listener, get_listener, get_statuscondition,
get_status_changes, enable, get_instance_handle.

**Repo:** `crates/dcps/src/entity.rs` (`Entity`, `EntityState`); implementiert
für `DomainParticipant` (`crates/dcps/src/participant.rs`), `Topic<T>`
(`crates/dcps/src/topic.rs`), `Publisher` und `DataWriter<T>`
(`crates/dcps/src/publisher.rs`), `Subscriber` und `DataReader<T>`
(`crates/dcps/src/subscriber.rs`).

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::entity_handles_unique_across_types`,
`crates/dcps/src/entity.rs::tests::entity_state_starts_disabled`

**Status:** done — `set_listener` und `get_listener` sind keine Methoden des
Traits, sondern je Entity-Typ definiert, weil der Listener-Typ je Entity
verschieden ist; die Spec lässt das zu ("must be provided by each of the
derived Entity classes").

### §2.2.2.1.1 StatusKind

**Spec:** §2.2.2.1.1, S. 24 — "StatusKind is an enumerated type that identifies
each concrete Status type."

**Repo:** `crates/dcps/src/psm_constants.rs` (Modul `status`),
`crates/dcps/src/entity.rs` (`StatusMask`)

**Tests:** `crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`

**Status:** done — als Bitmaske nach der IDL-PSM (§2.3.3); die Werte sind unter
§2.3.3 einzeln geführt.

### §2.2.2.1.1.1 set_qos — je abgeleiteter Entity, Werte ersetzen die bestehenden

**Spec:** §2.2.2.1.1.1, S. 24 — "This operation must be provided by each of the
derived Entity classes (DomainParticipant, Topic, Publisher, DataWriter,
Subscriber, DataReader) … The set of policies specified as the qos_list
parameter are applied on top of the existing QoS, replacing the values of any
policies previously set."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::set_qos`); Implementierungen in
`crates/dcps/src/participant.rs`, `crates/dcps/src/topic.rs`,
`crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`,
`crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`

**Status:** done — die Rust-API nimmt den vollständigen QoS-Satz der Entity;
er ersetzt den bestehenden.

### §2.2.2.1.1.1 set_qos — IMMUTABLE_POLICY nach enable

**Spec:** §2.2.2.1.1.1, S. 24 — "If set_qos is invoked after the Entity is
enabled and it attempts to change the value of an “immutable” policy, the
operation will fail and it returns IMMUTABLE_POLICY."

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`);
`crates/dcps/src/publisher.rs` (`Publisher::set_qos`: PRESENTATION);
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`: PRESENTATION).

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`

**Status:** done — die Liste der unveränderlichen Policies je Entity entspricht
der Spalte „Changeable“ der QoS-Tabelle (§2.2.3); DataWriter und DataReader
führen zusätzlich PRESENTATION und DATA_REPRESENTATION, der DataReader
TYPE_CONSISTENCY_ENFORCEMENT (XTypes 1.3).

### §2.2.2.1.1.1 set_qos — INCONSISTENT_POLICY

**Spec:** §2.2.2.1.1.1, S. 24 — "The set_qos operation will also fail if it
specifies a set of values that once combined with the existing values would
result in an inconsistent set of policies. In this case, the return value is
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`

**Status:** `done` — ein `set_qos` mit widersprüchlichen Policies liefert
INCONSISTENT_POLICY und lässt die QoS, wie sie war.

### §2.2.2.1.1.1 set_qos — UNSUPPORTED bei nicht unterstütztem Wert

**Spec:** §2.2.2.1.1.1, S. 24 — "If the application supplies a non-default
value for a QoS policy that is not supported by the implementation of the
service, the set_qos operation will fail and return UNSUPPORTED."

**Repo:** `crates/dcps/src/publisher.rs` (`validate_writer_data_representation`), `crates/dcps/src/runtime.rs` (`check_emittable_offer`)

**Tests:** `crates/dcps/tests/unsupported_qos.rs::a_data_representation_the_service_does_not_emit_is_unsupported`

**Status:** done — ein DATA_REPRESENTATION-Wert, den der Dienst nicht aussendet, liefert `Unsupported` (Rückgabecode 2), und der Writer entsteht nicht. Werte, die nur teilweise ausgeführt wurden, gibt es nicht mehr: OWNERSHIP EXCLUSIVE wechselt den Eigentümer (§2.2.3.9.2), TRANSIENT und PERSISTENT haben die fiktiven Entities (§2.2.3.4).

### §2.2.2.1.1.1 set_qos — keine Änderung bei Fehlschlag

**Spec:** §2.2.2.1.1.1, S. 24 — "The existing set of policies are only changed
if the set_qos operation succeeds. This is indicated by the OK return value. In
all other cases, none of the policies is modified."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_qos`),
`crates/dcps/src/subscriber.rs` (`DataReader::set_qos`),
`crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`,
`crates/dcps/tests/set_qos_atomic.rs::a_refused_set_qos_leaves_the_entity_factory_of_publisher_and_subscriber`

**Status:** done — DataWriter, DataReader und Topic weisen vor jeder Zuweisung ab; Publisher und Subscriber setzen ENTITY_FACTORY nach der Prüfung der neuen PARTITION gegen die Laufzeit (nächstes Item).

### §2.2.2.1.1.1 set_qos — keine Änderung bei Fehlschlag: Publisher und Subscriber

**Spec:** §2.2.2.1.1.1, S. 24 — "In all other cases, none of the policies is
modified."

**Repo:** —

**Tests:** `crates/dcps/tests/set_qos_atomic.rs::a_refused_set_qos_leaves_the_entity_factory_of_publisher_and_subscriber`

**Status:** done — ein `set_qos`, das ENTITY_FACTORY und die unveränderliche PRESENTATION zugleich ändert, wird abgewiesen, und ENTITY_FACTORY bleibt (ein danach erzeugter Writer und Reader ist freigegeben). Die Ablehnung der PARTITION durch die Laufzeit gibt es nur unter DDS-Security (`DcpsRuntime::check_user_group_partition`); dieser Weg ist gelesen, nicht gemessen.

### §2.2.2.1.1.1 set_qos — Sonderwert *_QOS_DEFAULT

**Spec:** §2.2.2.1.1.1, S. 24–25 — "This special value may be used as a
parameter to the set_qos operation to indicate that the QoS of the Entity should
be changed to match the current default QoS set in the Entity’s factory."

**Repo:** `crates/dcps/src/qos.rs` (`QosArg`), `crates/dcps/src/entity.rs`
(`Entity::set_qos`), die sechs `set_qos`;
`crates/zerodds-c-api/src/extra_ffi.rs::current_default`

**Tests:** `crates/dcps/tests/default_qos.rs::the_participant_factory_default`,
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`,
`crates/dcps/tests/default_qos.rs::set_qos_with_the_default_keeps_the_immutable_rule`,
`crates/dcps/tests/default_qos.rs::set_qos_with_the_default_on_topic_subscriber_and_reader`,
`crates/zerodds-c-api/src/extra_ffi.rs::tests::set_qos_with_null_takes_the_current_default_of_the_factory`

**Status:** `done` — `set_qos(*_QOS_DEFAULT)` nimmt den aktuellen Default der
Factory der Entity und wendet ihn unter denselben Regeln an wie denselben Wert;
nach der Freigabe ist eine abweichende unveränderliche Policy IMMUTABLE_POLICY.
Im C-API steht `NULL` für den Sonderwert.

### §2.2.2.1.1.2 get_qos

**Spec:** §2.2.2.1.1.2, S. 25 — "This operation allows access to the existing
set of QoS policies for the Entity."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_qos`); Implementierungen je
Entity.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::topic_qos_get_returns_clone`

**Status:** done

### §2.2.2.1.1.3 set_listener — Listener mit Maske installieren

**Spec:** §2.2.2.1.1.3, S. 25 — "This operation installs a Listener on the
Entity. The listener will only be invoked on the changes of communication status
indicated by the specified mask."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_listener`),
`crates/dcps/src/topic.rs` (`Topic::set_listener`),
`crates/dcps/src/publisher.rs` (`Publisher::set_listener`,
`DataWriter::set_listener`), `crates/dcps/src/subscriber.rs`
(`Subscriber::set_listener`, `DataReader::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_with_only_data_available_bit_does_not_call_reader_for_others`,
`crates/dcps/tests/listener_integration.rs::push_raw_with_zero_mask_at_reader_bubbles_to_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::reader_with_only_data_available_bit_passes_other_events_up`

**Status:** done

### §2.2.2.1.1.3 set_listener — nil-Listener

**Spec:** §2.2.2.1.1.3, S. 25 — "It is permitted to use ‘nil’ as the value of
the listener. The ‘nil’ listener behaves as a Listener whose operations perform
no action."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` mit nicht leerer Maske setzt `NilListener`: die Status der Maske werden ohne Wirkung behandelt, gehen nicht nach oben (bei DATA_ON_READERS folgt kein `on_data_available`) und werden nicht zurückgesetzt. `get_listener` liefert dafür keinen. Eine leere Maske ist kein Listener. Die C-ABI (gepollte Listener) nimmt NULL mit Maske ebenso als nil-Listener: `zerodds_poll_listeners` lässt in den Ebenen Publisher, Subscriber und Participant die Status weg, die ein nil-Listener von Writer, Reader, Subscriber oder Topic darunter in seiner Maske hat (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.2.1.1.3 set_listener — ein Listener je Entity, Ersetzen

**Spec:** §2.2.2.1.1.3, S. 25 — "Only one listener can be attached to each
Entity. If a listener was already set, the operation set_listener will replace
it with the new one. Consequently if the value ‘nil’ is passed for the listener
parameter to the set_listener operation, any existing listener will be removed."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_listener`:
ein Platz `Option<(Listener, StatusMask)>`); entsprechend je Entity.

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_listener_replacement_works`,
`crates/dcps/tests/listener_integration.rs::participant_listener_replacement_works`,
`crates/dcps/tests/listener_integration.rs::listener_replacement_during_dispatch_is_safe`

**Status:** done

### §2.2.2.1.1.4 get_listener

**Spec:** §2.2.2.1.1.4, S. 25 — "This operation allows access to the existing
Listener attached to the Entity."

**Repo:** `get_listener` je Entity in `crates/dcps/src/participant.rs`,
`crates/dcps/src/topic.rs`, `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`.

**Tests:** `crates/dcps/tests/listener_integration.rs::participant_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::topic_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.1.1.5 get_statuscondition

**Spec:** §2.2.2.1.1.5, S. 25 — "This operation allows access to the
StatusCondition … associated with the Entity. The returned condition can then be
added to a WaitSet". Nach Figure 2.5 und §2.2.2.1.9 gehört zu jeder Entity
genau eine StatusCondition.

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_status_condition`, `EntityState`)

**Tests:** `crates/dcps/tests/status_condition.rs::an_entity_has_one_status_condition`, `crates/dcps/tests/status_condition.rs::get_entity_returns_the_entity_of_each_kind`,
`crates/dcps/tests/entity_lifecycle.rs::status_condition_trigger_on_relevant_status`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done — jeder Aufruf liefert die eine StatusCondition der Entity; die darüber gesetzten `enabled_statuses` gelten für die Entity. Sie verweist schwach auf ihre Entity.

### §2.2.2.1.1.6 get_status_changes — ausgelöste Status

**Spec:** §2.2.2.1.1.6, S. 25 — "This operation retrieves the list of
communication statuses in the Entity that are ‘triggered.’ That is, the list of
statuses whose value has changed since the last time the application read the
status."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_status_changes`,
`EntityState::status_changes`, `EntityState::clear_status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/entity_lifecycle.rs::datareader_status_changes_track`

**Status:** done

### §2.2.2.1.1.6 get_status_changes — leer nach dem Erzeugen

**Spec:** §2.2.2.1.1.6, S. 25 — "When the entity is first created or if the
entity is not enabled, all communication statuses are in the “untriggered”
state so the list returned by the get_status_changes operation will be empty."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::with_enabled`:
`status_changes = 0`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datareader_status_changes_track`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — nach dem Erzeugen und solange nicht freigegeben ist die Liste leer (`EntityState::status_changes` liefert 0 für eine nicht freigegebene Entity).

### §2.2.2.1.1.6 get_status_changes — leer, solange nicht freigegeben

**Spec:** §2.2.2.1.1.6, S. 25 — "… or if the entity is not enabled, all
communication statuses are in the “untriggered” state".

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::status_changes`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — Participant und Reader einer nicht freigegebenen Factory melden `get_status_changes() == 0`.

### §2.2.2.1.1.6 get_status_changes — nur die eigenen Status

**Spec:** §2.2.2.1.1.6, S. 25 — "The list of statuses returned by the
get_status_changes operation refers to the status that are triggered on the
Entity itself and does not include statuses that apply to contained entities."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::get_status_changes_reports_the_entity_itself`

**Status:** done — Writer und Reader melden PUBLICATION_MATCHED bzw. SUBSCRIPTION_MATCHED, Publisher, Subscriber und Participant melden sie nicht.

### §2.2.2.1.1.7 enable — freigeben, idempotent

**Spec:** §2.2.2.1.1.7, S. 26 — "This operation enables the Entity … The enable
operation is idempotent. Calling enable on an already enabled Entity returns OK
and has no effect."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`,
`Entity::enable`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_is_enabled_by_the_factory_and_enable_is_idempotent`,
`crates/dcps/src/entity.rs::tests::enable_is_idempotent_and_reports_first_transition`

**Status:** done

### §2.2.2.1.1.7 enable — Default von ENTITY_FACTORY

**Spec:** §2.2.2.1.1.7, S. 26 — "Entity objects can be created either enabled
or disabled. This is controlled by the value of the ENTITY_FACTORY Qos policy …
by default, it is not necessary to explicitly call enable on newly created
entities".

**Repo:** `crates/dcps/src/participant.rs` (`autoenables_children`),
`crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`),
`crates/dcps/src/factory.rs` (`DomainParticipantFactory::autoenable`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done

### §2.2.2.1.1.7 enable — Operationen auf einer nicht freigegebenen Entity

**Spec:** §2.2.2.1.1.7, S. 26 — vor `enable` zulässig sind QoS- und
Listener-Operationen (auch Default-QoS), get_statuscondition,
Factory-Operationen, get_status_changes "and other get status operations",
Lookup-Operationen; "Other operations may explicitly state that they may be
called on disabled entities; those that do not will return the error
NOT_ENABLED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_enabled`);
aufgerufen in den Daten-Operationen von `DataWriter` und `DataReader`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — QoS-, Listener-, Default-QoS-, StatusCondition-, Factory-, Lookup- und get-status-Operationen laufen auf einer nicht freigegebenen Entity; die übrigen (Daten-Operationen, `assert_liveliness`, `get_discovered_*`, `contains_entity`, `ignore_*`, `get_current_time`, Gruppenzugriff) liefern NOT_ENABLED.

### §2.2.2.1.1.7 enable — get-status-Operationen auf einer nicht freigegebenen Entity

**Spec:** §2.2.2.1.1.7, S. 26 — zulässig vor `enable`: "get_status_changes and
other get status operations (although the status of a disabled entity never
changes)".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs` (`get_*_status`), `crates/dcps/src/topic.rs` (`Topic::get_inconsistent_topic_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — die `get_*_status`-Operationen von DataWriter, DataReader und Topic folgen §2.2.2.1.1.7: auf einer nicht freigegebenen Entity liefern sie den nicht ausgelösten Status (alles null), wie es die C-ABI schon tat; eine gelöschte Entity ist `AlreadyDeleted`. Die Klassenbeschreibungen von Topic, Publisher, DataWriter, Subscriber und DataReader (S. 49, 56, 62, 78, 88) nennen nur die Basisoperationen als Ausnahme von NOT_ENABLED; der Widerspruch ist in der Befunddatei geführt, die ausdrückliche Liste von §2.2.2.1.1.7 entscheidet. `matched_subscriptions` und die übrigen Operationen, die §2.2.2.1.1.7 nicht nennt, bleiben NOT_ENABLED.

### §2.2.2.1.1.7 enable — NOT_ENABLED auf DomainParticipant, Publisher, Subscriber, Topic

**Spec:** §2.2.2.1.1.7, S. 26 — "those that do not will return the error
NOT_ENABLED."; §2.2.2.2.1, S. 35 — "Other operations will have the value
NOT_ENABLED if called on a disabled DomainParticipant"; §2.2.2.4.1, S. 56,
und §2.2.2.5.2, S. 78 — dieselbe Regel für Publisher und Subscriber.

**Repo:** `crates/dcps/src/participant.rs` (`assert_liveliness`, `try_get_discovered_participants`, `try_get_discovered_topics`, `try_contains_entity`, `get_discovered_*_data`)

**Tests:**
`crates/dcps/tests/entity_lifecycle.rs::group_operations_of_a_disabled_publisher_or_subscriber_are_not_enabled`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — die Operationen, die §2.2.2.1.1.7 und §2.2.2.2.1 nicht zulassen, liefern NOT_ENABLED; `get_discovered_participants`, `get_discovered_topics` und `contains_entity` haben dafür die Formen mit `Result` (`try_*`).

### §2.2.2.1.1.7 enable — Löschen einer nicht freigegebenen Entity

**Spec:** §2.2.2.1.1.7, S. 26 — "It is legal to delete an Entity that has not
been enabled by calling the proper operation on its factory."

**Repo:** —

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — Writer, Reader, Publisher, Subscriber und Topic einer nicht freigegebenen Factory werden gelöscht.

### §2.2.2.1.1.7 enable — von einer nicht freigegebenen Factory erzeugt

**Spec:** §2.2.2.1.1.7, S. 26 — "Entities created from a factory that is
disabled, are created disabled regardless of the setting of the ENTITY_FACTORY
Qos policy."

**Repo:** `crates/dcps/src/participant.rs` (`autoenables_children`),
`crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`

**Status:** done

### §2.2.2.1.1.7 enable — PRECONDITION_NOT_MET bei nicht freigegebener Factory

**Spec:** §2.2.2.1.1.7, S. 26 — "Calling enable on an Entity whose factory is
not enabled will fail and return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** done

### §2.2.2.1.1.7 enable — Freigabe der Factory gibt das Erzeugte frei

**Spec:** §2.2.2.1.1.7, S. 26 — "If the ENTITY_FACTORY Qos policy has
autoenable_created_entities set to TRUE, the enable operation on the factory
will automatically enable all entities created from the factory."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`, Kaskade
über `children`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`,
`crates/dcps/tests/enable_after_create.rs::a_participant_created_disabled_is_discovered_after_its_enable`

**Status:** done

### §2.2.2.1.1.7 enable — Listener erst nach der Freigabe

**Spec:** §2.2.2.1.1.7, S. 26 — "The Listeners associated with an entity are
not called until the entity is enabled."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`), `crates/dcps/src/subscriber.rs` (`notify_data_arrived`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — der Listener eines nicht freigegebenen Readers wird nicht gerufen; nach `enable` ruft ihn das nächste Sample.

### §2.2.2.1.1.7 enable — Conditions einer nicht freigegebenen Entity inaktiv

**Spec:** §2.2.2.1.1.7, S. 26 — "Conditions associated with an entity that is
not enabled are “inactive,” that is, have a trigger_value==FALSE".

**Repo:** `crates/dcps/src/subscriber.rs` (`holds_matching`), `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value` über `status_changes`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — ReadCondition und StatusCondition eines nicht freigegebenen Readers haben `trigger_value` FALSE, nach `enable` trifft die ReadCondition zu.

### §2.2.2.1.1.8 get_instance_handle

**Spec:** §2.2.2.1.1.8, S. 26 — "This operation returns the InstanceHandle_t
that represents the Entity."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_instance_handle`,
`EntityState::instance_handle`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_get_instance_handle_unique`,
`crates/dcps/tests/entity_lifecycle.rs::entity_handles_unique_across_types`,
`crates/dcps/src/entity.rs::tests::instance_handles_are_unique_per_entity`

**Status:** done — der Handle ist ein lokaler Zähler. Welcher Handle in
Built-in-Topics und Status für dieselbe Entity erscheint, ist unter §2.2.5
geführt.

### §2.2.2.1.2 DomainEntity

**Spec:** §2.2.2.1.2, S. 26 — "DomainEntity is the abstract base class for all
DCPS entities, except for the DomainParticipant. Its sole purpose is to express
that DomainParticipant is a special kind of Entity, which acts as a container of
all other Entity, but itself cannot contain other DomainParticipant."

**Repo:** —

**Tests:** `crates/dcps/tests/domain_entity.rs::a_domain_participant_contains_no_domain_participant`

**Status:** done — der Test legt zwei Participants an: keiner enthält den anderen, und alles, was einer erzeugt, gehört ihm und keinem anderen. Die Rust-API und das C-API haben keine Operation, die einen DomainParticipant in einem anderen erzeugt.

### §2.2.2.1.3 QosPolicy — Attribut name

**Spec:** §2.2.2.1.3, S. 27 — "It has an attribute name that is used to
identify uniquely each QoS policy. All concrete QosPolicy classes derive from
this root and include a value whose type depends on the concrete QoS policy."

**Repo:** `crates/qos/src/policies/name.rs` (`QosPolicy`, `policy_name`)

**Tests:** `crates/qos/src/policies/name.rs::tests::every_policy_has_its_name`

**Status:** done — `QosPolicy::name` jeder der 22 Policies liefert den Namen der Konstante der IDL; die 22 Namen sind verschieden. `GROUPDATA_QOS_POLICY_NAME` hat in der IDL den Wert `"TransportPriority"` (ein Versehen der Spec); hier ist es `"GroupData"`, und `"TransportPriority"` ist der Name von TRANSPORT_PRIORITY.

### §2.2.2.1.3 QosPolicy — je Entity eigene Policies

**Spec:** §2.2.2.1.3, S. 27 — "Each Entity can be configured with a list of
QosPolicy. However, any Entity cannot support any QosPolicy."

**Repo:** `crates/dcps/src/qos.rs` (`DomainParticipantQos`, `TopicQos`,
`PublisherQos`, `SubscriberQos`, `DataWriterQos`, `DataReaderQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::topic_qos_get_returns_clone`

**Status:** done — welche Policy welcher Entity zugeordnet ist, steht je Policy
unter §2.2.3.

### §2.2.2.1.3 QosPolicy — Konsistenzprüfung bei jeder Änderung

**Spec:** §2.2.2.1.3, S. 27 — "Consistency checking is performed each time the
policies are modified via the set_qos operation."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — jedes `set_qos` und `create_*` von Topic, DataWriter und
DataReader prüft die Konsistenz.

### §2.2.2.1.3 QosPolicy — Übergang und unveränderliche Policies

**Spec:** §2.2.2.1.3, S. 27 — "When a policy is changed after being set to a
given value, it is not required that the new value be applied instantaneously;
the Service is allowed to apply it after a transition phase. In addition, some
QosPolicy have “immutable” semantics".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis ohne Pflicht; die unveränderlichen
Policies sind unter §2.2.2.1.1.1 „IMMUTABLE_POLICY“ und je Policy unter §2.2.3
geführt.

### §2.2.2.1.4 Listener — Wurzel der Listener-Schnittstellen

**Spec:** §2.2.2.1.4, S. 27 — "Listener is the abstract root for all Listener
interfaces. All the supported kinds of concrete Listener interfaces (one per
concrete Entity: DomainParticipant, Topic, Publisher, DataWriter, Subscriber,
and DataReader) derive from this root".

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener`, `DataWriterListener`,
`PublisherListener`, `DataReaderListener`, `SubscriberListener`,
`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::topic_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — sechs Listener-Traits, einer je Entity-Art; ein
gemeinsamer Basistrait ohne Methoden fehlt, er hätte keine Operation. Die
Callback-Signaturen sind unter §2.2.4.3 geführt.

### §2.2.2.1.5 Status — Wurzel der Status-Klassen

**Spec:** §2.2.2.1.5, S. 28 — "Status is the abstract root class for all
communication status objects … Each concrete Entity is associated with a set of
Status objects whose value represents the “communication status” of that
entity. These status values can be accessed with corresponding methods on the
Entity."

**Repo:** `crates/dcps/src/status.rs` (ein Rust-Typ je Status)

**Tests:** `crates/dcps/src/status.rs::tests::inconsistent_topic_default_is_zero`,
`crates/dcps/src/status.rs::tests::data_available_and_data_on_readers_are_zero_sized_markers`

**Status:** done — die Status und ihre Felder sind unter §2.2.4.1 einzeln
geführt.

### §2.2.2.1.6 WaitSet — ohne Factory erzeugbar

**Spec:** §2.2.2.1.6, S. 28 — "WaitSet has no factory. It is created as an
object directly by the natural means in each language binding … it is not
necessarily associated with a single DomainParticipant".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::new`, `WaitSet::default`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_default_is_empty`,
`crates/dcps/src/condition.rs::tests::waitset_with_multiple_conditions_returns_all_triggered`

**Status:** done

### §2.2.2.1.6.1 attach_condition — anhängen

**Spec:** §2.2.2.1.6.1, S. 28 — "Attaches a Condition to the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::read_condition_attaches_to_waitset`

**Status:** done

### §2.2.2.1.6.1 attach_condition — während wait

**Spec:** §2.2.2.1.6.1, S. 28 — "It is possible to attach a Condition on a
WaitSet that is currently being waited upon (via the wait operation). In this
case, if the Condition has a trigger_value of TRUE, then attaching the condition
will unblock the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`:
`signal_data_available`)

**Tests:** `crates/dcps/tests/waitset_attach.rs::attaching_a_triggered_condition_unblocks_a_waiting_waitset`

**Status:** `done` — eine angehängte Condition weckt ein wartendes `wait`;
gemessen 0,10 s statt 5 s bis zur Frist.

### §2.2.2.1.6.1 attach_condition — bereits angehängt

**Spec:** §2.2.2.1.6.1, S. 29 — "Adding a Condition that is already attached to
the WaitSet has no effect."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`:
`Arc::ptr_eq`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::waitset_attach_idempotent_does_not_count_against_cap`

**Status:** done

### §2.2.2.1.6.1 attach_condition — OUT_OF_RESOURCES

**Spec:** §2.2.2.1.6.1, S. 29 — "Possible error codes returned in addition to
the standard ones: OUT_OF_RESOURCES."

**Repo:** `crates/dcps/src/condition.rs` (`MAX_WAITSET_CONDITIONS`,
`WaitSet::attach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_above_max_returns_out_of_resources`

**Status:** done

### §2.2.2.1.6.2 detach_condition — abhängen

**Spec:** §2.2.2.1.6.2, S. 29 — "Detaches a Condition from the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::detach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`, `crates/dcps/tests/status_condition.rs::detach_condition_wants_an_attached_condition`

**Status:** done — eine nicht angehängte Condition liefert `PreconditionNotMet`.

### §2.2.2.1.6.3 wait — blockieren bis eine Condition auslöst

**Spec:** §2.2.2.1.6.3, S. 29 — "If none of the conditions attached to the
WaitSet have a trigger_value of TRUE, the wait operation will block suspending
the calling thread. The result of the wait operation is the list of all the
attached conditions that have a trigger_value of TRUE".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`,
`WaitSet::poll_active`, `signal_data_available`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_returns_immediately_if_already_triggered`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`,
`crates/dcps/src/condition.rs::tests::waitset_with_multiple_conditions_returns_all_triggered`,
`crates/dcps/src/condition.rs::tests::data_signal_wakes_a_blocked_wait`,
`crates/dcps/tests/waitset_notification.rs::data_arrival_unblocks_a_waitset_waiting_on_a_read_condition`

**Status:** done

### §2.2.2.1.6.3 wait — TIMEOUT

**Spec:** §2.2.2.1.6.3, S. 29 — "It this duration is exceeded and none of the
attached Condition objects is true, wait will return with the return code
TIMEOUT."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_timeout_returns_err`

**Status:** done

### §2.2.2.1.6.3 wait — ein wartender Thread, sonst PRECONDITION_NOT_MET

**Spec:** §2.2.2.1.6.3, S. 29 — "If the wait operation is invoked on a WaitSet
that already has a thread blocking on it, the operation will return immediately
with the value PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSetInner::waiting`,
`WaitSet::wait`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_concurrent_wait_returns_precondition_not_met`,
`crates/dcps/src/condition.rs::tests::waitset_sequential_waits_after_first_returns_succeed`

**Status:** done

### §2.2.2.1.6.4 get_conditions

**Spec:** §2.2.2.1.6.4, S. 29 — "This operation retrieves the list of attached
conditions."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::get_conditions`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::waitset_clone_shares_state`

**Status:** done

### §2.2.2.1.7 Condition — trigger_value

**Spec:** §2.2.2.1.7, S. 29 — "A Condition is a root class for all the
conditions that may be attached to a WaitSet … A Condition has a trigger_value
that can be TRUE or FALSE and is set automatically by the Service."

**Repo:** `crates/dcps/src/condition.rs` (`Condition`); implementiert für
`GuardCondition`, `StatusCondition`, `ReadCondition`, `QueryCondition`.

**Tests:** `crates/dcps/src/condition.rs::tests::status_condition_implements_condition_trait`,
`crates/dcps/src/condition.rs::tests::read_condition_implements_condition_trait_object_safe`

**Status:** done

### §2.2.2.1.7.1 get_trigger_value

**Spec:** §2.2.2.1.7.1, S. 29 — "This operation retrieves the trigger_value of
the Condition."

**Repo:** `crates/dcps/src/condition.rs` (`Condition::get_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`,
`crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.1.8 GuardCondition — ohne Factory, Anfangswert FALSE

**Spec:** §2.2.2.1.8, S. 30 — "GuardCondition has no factory. It is created as
an object directly by the natural means in each language binding … When first
created the trigger_value is set to FALSE."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::new`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_starts_false`

**Status:** done

### §2.2.2.1.8.1 set_trigger_value

**Spec:** §2.2.2.1.8.1, S. 30 — "This operation sets the trigger_value of the
GuardCondition. WaitSet objects behavior depends on the changes of the
trigger_value of their attached conditions."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::set_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`

**Status:** done — ein wartendes WaitSet sieht die Änderung nach höchstens
`poll_interval` (1 ms); `set_trigger_value` selbst sendet kein Signal.

### §2.2.2.1.9 StatusCondition — trigger_value gefiltert durch enabled_statuses

**Spec:** §2.2.2.1.9, S. 30 — "The trigger_value of the StatusCondition depends
on the communication status of that entity …, ‘filtered’ by the set of
enabled_statuses on the StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/entity_lifecycle.rs::status_condition_trigger_on_relevant_status`

**Status:** done

### §2.2.2.1.9.1 set_enabled_statuses

**Spec:** §2.2.2.1.9.1, S. 30–31 — "This operation defines the list of
communication statuses that are taken into account to determine the
trigger_value of the StatusCondition. This operation may change the
trigger_value of the StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::set_enabled_statuses`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done — die Maske gilt für das eine `StatusCondition`-Objekt; siehe
§2.2.2.1.1.5.

### §2.2.2.1.9.1 set_enabled_statuses — Default: alle Status

**Spec:** §2.2.2.1.9.1, S. 31 — "If this function is not invoked, the default
list of enabled statuses includes all the statuses."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_status_condition_starts_with_every_status_enabled`

**Status:** done — der Test deckt es ab.

### §2.2.2.1.9.2 get_enabled_statuses

**Spec:** §2.2.2.1.9.2, S. 31 — "This operation returns the statuses that were
explicitly set on the last call to set_enabled_statuses or, if
set_enabled_statuses was never called, the default list".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_status_condition_starts_with_every_status_enabled`

**Status:** done — der Test deckt es ab.

### §2.2.2.1.9.3 get_entity

**Spec:** §2.2.2.1.9.3, S. 31 — "This operation returns the Entity associated
with the StatusCondition. Note that there is exactly one Entity associated with
each StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::get_entity`)

**Tests:** `crates/dcps/tests/status_condition.rs::get_entity_returns_the_entity_of_each_kind`, `crates/dcps/tests/status_condition.rs::get_entity_is_none_once_the_entity_is_gone`

**Status:** done — `get_entity` liefert den `EntityRef` der Entity (Participant, Publisher, Subscriber, Topic, Writer oder Reader); `None`, wenn die Entity nicht mehr existiert.

---

## §2.2.2.2 Domain Module

### §2.2.2.2 Klassen des Domain Module

**Spec:** §2.2.2.2, S. 32 — DomainParticipant, DomainParticipantFactory,
DomainParticipantListener.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Klassen sind unten einzeln
geführt.

### §2.2.2.2.1 DomainParticipant — Container, Factory, Domain

**Spec:** §2.2.2.2.1, S. 32–33 — der DomainParticipant ist Container aller
anderen Entities, Factory für Publisher, Subscriber, Topic und MultiTopic, und
steht für die Teilnahme an einem Domain: "A domain establishes a “virtual
network” linking all applications that share the same domainId and isolating
them from applications running on different domains."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant`,
`ParticipantInner`); Domain-Trennung in `crates/dcps/src/runtime.rs`
(SPDP-Ports und Domain-Prüfung der Ankündigung).

**Tests:** `crates/dcps/src/participant.rs::tests::participant_created_with_domain_id`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** done

### §2.2.2.2.1 Operationen auf einem nicht freigegebenen DomainParticipant

**Spec:** §2.2.2.2.1, S. 35 — "The following operations may be called even if
the DomainParticipant is not enabled. Other operations will have the value
NOT_ENABLED if called on a disabled DomainParticipant": set_qos, get_qos,
set_listener, get_listener, enable; create_topic, create_publisher,
create_subscriber, delete_topic, delete_publisher, delete_subscriber;
get_statuscondition.

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic`,
`DomainParticipant::create_publisher`, `DomainParticipant::create_subscriber`
ohne Zustandsprüfung)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — die genannten Operationen laufen auf einem nicht freigegebenen DomainParticipant, die anderen liefern NOT_ENABLED.

### §2.2.2.2.1.1 create_publisher — erzeugen

**Spec:** §2.2.2.2.1.1, S. 35–36 — "This operation creates a Publisher with the
desired QoS policies … If the specified QoS policies are not consistent, the
operation will fail and no Publisher will be created … The created Publisher
belongs to the DomainParticipant that is its factory. In case of failure, the
operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_publisher`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_publisher`,
`crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_of_another_participant`

**Status:** done — für `PublisherQos` (PRESENTATION, PARTITION, GROUP_DATA,
ENTITY_FACTORY) nennt §2.2.3 keine Kombination, die inkonsistent ist; die
Operation kann daher nicht fehlschlagen und liefert `Publisher` statt
`Result`.

### §2.2.2.2.1.1 create_publisher — Listener und Maske

**Spec:** §2.2.2.2.1.1, S. 35 — Parameter `a_listener: PublisherListener`,
`mask: StatusKind []`; "attaches to it the specified PublisherListener".

**Repo:** `create_publisher_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_publisher_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.2.1.1 create_publisher — PUBLISHER_QOS_DEFAULT

**Spec:** §2.2.2.2.1.1, S. 35 — "The special value PUBLISHER_QOS_DEFAULT can be
used to indicate that the Publisher should be created with the default
Publisher QoS set in the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_publisher`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — `PUBLISHER_QOS_DEFAULT` nimmt `get_default_publisher_qos`;
ein QoS-Wert wird übernommen, wie er ist.

### §2.2.2.2.1.2 delete_publisher — löschen

**Spec:** §2.2.2.2.1.2, S. 36 — "This operation deletes an existing Publisher."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`

**Status:** done — der Test löscht nach dem Entfernen des DataWriters und
prüft `AlreadyDeleted` danach.

### §2.2.2.2.1.2 delete_publisher — PRECONDITION_NOT_MET mit DataWriter

**Spec:** §2.2.2.2.1.2, S. 36 — "A Publisher cannot be deleted if it has any
attached DataWriter objects. If delete_publisher is called on a Publisher with
existing DataWriter object, it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`,
`Publisher::has_datawriters`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`

**Status:** done

### §2.2.2.2.1.2 delete_publisher — anderer DomainParticipant

**Spec:** §2.2.2.2.1.2, S. 36 — "If delete_publisher is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_of_another_participant`

**Status:** done

### §2.2.2.2.1.3 create_subscriber — erzeugen

**Spec:** §2.2.2.2.1.3, S. 36 — "This operation creates a Subscriber with the
desired QoS policies … The created Subscriber belongs to the DomainParticipant
that is its factory. In case of failure, the operation will return a ‘nil’
value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_subscriber`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_subscriber`,
`crates/dcps/tests/e2e_dcps_api.rs::public_api_create_subscriber_and_reader_offline`

**Status:** done — für `SubscriberQos` nennt §2.2.3 keine inkonsistente
Kombination; die Operation liefert `Subscriber` statt `Result`.

### §2.2.2.2.1.3 create_subscriber — Listener und Maske

**Spec:** §2.2.2.2.1.3, S. 36 — "attaches to it the specified
SubscriberListener"; Parameter `a_listener`, `mask`.

**Repo:** `create_subscriber_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_subscriber_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.2.1.3 create_subscriber — SUBSCRIBER_QOS_DEFAULT

**Spec:** §2.2.2.2.1.3, S. 36 — "The special value SUBSCRIBER_QOS_DEFAULT can be
used to indicate that the Subscriber should be created with the default
Subscriber QoS set in the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_subscriber`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — `SUBSCRIBER_QOS_DEFAULT` nimmt
`get_default_subscriber_qos`.

### §2.2.2.2.1.4 delete_subscriber — löschen

**Spec:** §2.2.2.2.1.4, S. 36 — "This operation deletes an existing Subscriber."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.4 delete_subscriber — PRECONDITION_NOT_MET mit DataReader

**Spec:** §2.2.2.2.1.4, S. 36 — "If the delete_subscriber operation is called
on a Subscriber with existing DataReader objects, it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`,
`Subscriber::has_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.4 delete_subscriber — anderer DomainParticipant

**Spec:** §2.2.2.2.1.4, S. 36 — "If delete_subscriber is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.5 create_topic — erzeugen

**Spec:** §2.2.2.2.1.5, S. 36–37 — "This operation creates a Topic with the
desired QoS policies … The created Topic belongs to the DomainParticipant that
is its factory … In case of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic`,
`DomainParticipant::create_topic_with_support`,
`DomainParticipant::create_serialized_topic`)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_stores_in_registry`,
`crates/dcps/src/participant.rs::tests::create_topic_rejects_empty_name`,
`crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_topic`

**Status:** done — ein Fehler ist `Err(DdsError)`.

### §2.2.2.2.1.5 create_topic — Listener und Maske

**Spec:** §2.2.2.2.1.5, S. 36 — "attaches to it the specified TopicListener";
Parameter `a_listener`, `mask`.

**Repo:** `create_topic_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_topic_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.2.1.5 create_topic — inkonsistente QoS

**Spec:** §2.2.2.2.1.5, S. 36 — "If the specified QoS policies are not
consistent, the operation will fail and no Topic will be created."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — `create_topic` lehnt eine TopicQos ab, deren HISTORY und
RESOURCE_LIMITS widersprechen.

### §2.2.2.2.1.5 create_topic — TOPIC_QOS_DEFAULT

**Spec:** §2.2.2.2.1.5, S. 36 — "The special value TOPIC_QOS_DEFAULT can be used
to indicate that the Topic should be created with the default Topic QoS set in
the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_topic`,
`::create_serialized_topic`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — `TOPIC_QOS_DEFAULT` nimmt `get_default_topic_qos`.

### §2.2.2.2.1.5 create_topic — an den registrierten Typ gebunden

**Spec:** §2.2.2.2.1.5, S. 37 — "The Topic is bound to a type described by the
type_name argument. Prior to creating a Topic the type must have been
registered with the Service. This is done using the register_type operation".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic_of_registered_type`, `create_topic::<T>`)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_takes_a_second_type_and_rejects_an_inconsistent_qos`,
`crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/dcps/tests/type_support.rs::a_topic_is_bound_to_the_registered_type`

**Status:** done — `create_topic_of_registered_type` verlangt, dass der Typ unter dem Namen registriert ist (PRECONDITION_NOT_MET sonst), und bindet das Topic an ihn. `create_topic::<T>` registriert `T` unter seinem eigenen Namen stillschweigend.

### §2.2.2.2.1.5 create_topic — mehrfach erhalten, mehrfach löschen

**Spec:** §2.2.2.2.1.11, S. 38 — "If a Topic is obtained multiple times by means
of find_topic or create_topic, it must also be deleted that same number of times
using delete_topic."

**Repo:** `crates/dcps/src/topic.rs` (`TopicInner::obtained`),
`crates/dcps/src/participant.rs` (`create_topic_with_support`, `delete_topic`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::a_topic_created_twice_goes_with_the_second_delete`

**Status:** `done` — jedes `create_topic` und jedes `find_topic` desselben Topic zählt; erst das letzte `delete_topic` löscht es (`tests/find_topic.rs::a_topic_obtained_by_find_topic_is_deleted_as_often`).

### §2.2.2.2.1.6 delete_topic — löschen

**Spec:** §2.2.2.2.1.6, S. 37 — "This operation deletes a Topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`,
`DomainParticipant::delete_topic_by_name`,
`DomainParticipant::delete_registered_topic`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_succeeds_when_nothing_uses_it`,
`crates/dcps/tests/delete_topic.rs::a_deleted_topic_is_gone_from_the_registry_and_its_name_is_free_again`,
`crates/dcps/src/participant.rs::tests::delete_topic_removes_the_entry_and_its_handle`

**Status:** done

### §2.2.2.2.1.6 delete_topic — PRECONDITION_NOT_MET bei Benutzern

**Spec:** §2.2.2.2.1.6, S. 37 — "The deletion of a Topic is not allowed if there
are any existing DataReader, DataWriter, ContentFilteredTopic, or MultiTopic
objects that are using the Topic … it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`ParticipantInner::topic_users`,
`DomainParticipant::delete_registered_topic`); `crates/dcps/src/topic.rs`
(`TopicUserCounts`, `Topic::attach_user`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_datawriter_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_datareader_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_content_filtered_topic_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_multitopic_uses_it`,
`crates/dcps/tests/delete_topic.rs::every_kind_of_user_holds_the_topic_and_releasing_all_of_them_frees_it`

**Status:** done

### §2.2.2.2.1.6 delete_topic — anderer DomainParticipant

**Spec:** §2.2.2.2.1.6, S. 37 — "If delete_topic is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_from_another_participant_is_refused_and_changes_nothing`

**Status:** done

### §2.2.2.2.1.7 create_contentfilteredtopic — erzeugen

**Spec:** §2.2.2.2.1.7, S. 37 — "The related Topic being subscribed to is
specified by means of the related_topic parameter … The logical expression is
derived from the filter_expression and expression_parameters arguments … In case
of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::create_contentfilteredtopic`); `crates/dcps/src/topic.rs`
(`ContentFilteredTopic::new`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_name`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_expression`

**Status:** done — ein nicht lesbarer Ausdruck oder ein `%n` ohne Parameter
liefert `BadParameter`. Die Grammatik ist unter Annex B geführt.

### §2.2.2.2.1.8 delete_contentfilteredtopic — löschen

**Spec:** §2.2.2.2.1.8, S. 37 — "This operation deletes a ContentFilteredTopic."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`;
`crates/dcps/src/topic.rs` (`DescriptionLife`)

**Tests:** `crates/dcps/tests/delete_description.rs::a_deleted_content_filtered_topic_is_deleted_for_every_handle`,
`crates/dcps/tests/delete_description.rs::a_reader_cannot_be_created_on_a_deleted_content_filtered_topic`

**Status:** `done` — das Löschen gilt für jedes Handle: das ContentFilteredTopic hält sein Topic nicht mehr (`delete_topic` geht danach), ein zweites Löschen und jede weitere Operation liefern `AlreadyDeleted`.

### §2.2.2.2.1.8 delete_contentfilteredtopic — PRECONDITION_NOT_MET bei DataReadern

**Spec:** §2.2.2.2.1.8, S. 37 — "If the delete_contentfilteredtopic operation is
called on a ContentFilteredTopic with existing DataReader objects attached to
it, it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`;
`crates/dcps/src/subscriber.rs::Subscriber::create_datareader_with_cft`;
`crates/zerodds-c-api/src/participant_ffi.rs::zerodds_dp_delete_contentfilteredtopic`

**Tests:** `crates/dcps/tests/delete_description.rs::a_content_filtered_topic_with_a_reader_is_not_deleted`,
`crates/dcps/tests/delete_description.rs::a_dropped_reader_lets_the_content_filtered_topic_go`,
`crates/zerodds-c-api/src/subscriber_ffi.rs::tests::a_cft_with_a_reader_is_not_deleted`

**Status:** `done` — solange ein DataReader auf dem ContentFilteredTopic besteht, liefert das Löschen `PreconditionNotMet`; mit dem Löschen des Readers (auch durch Ende seines letzten Handles) ist es erlaubt. Im C-API ebenso.

### §2.2.2.2.1.8 delete_contentfilteredtopic — anderer DomainParticipant

**Spec:** §2.2.2.2.1.8, S. 37 — "If delete_contentfilteredtopic is called on a
different DomainParticipant, the operation will have no effect and it will
return PRECONDITION_NOT_MET."

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`

**Tests:**
`crates/dcps/src/participant.rs::tests::delete_contentfilteredtopic_rejects_foreign`

**Status:** `done` — `PreconditionNotMet`, ohne Wirkung.

### §2.2.2.2.1.9 create_multitopic — erzeugen

**Spec:** §2.2.2.2.1.9, S. 37–38 — "a MultiTopic can be used to subscribe to
multiple topics and combine/filter the received data into a resulting type …
The list of topics and the logic used to combine filter and re-arrange the
information from each Topic are specified using the subscription_expression and
expression_parameters arguments."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_multitopic`,
`crates/dcps/src/multitopic.rs::MultiTopicPlan::new`

**Tests:** `crates/dcps/tests/multitopic.rs::a_subscription_expression_must_be_a_topic_expression`,
`crates/dcps/tests/multitopic.rs::the_topics_of_the_from_clause_must_exist_and_have_a_type_object`,
`crates/dcps/tests/multitopic.rs::topics_sharing_a_member_name_need_the_same_type`,
`crates/dcps/tests/multitopic.rs::the_result_type_must_hold_the_fields_of_the_select_list`

**Status:** `done` — `create_multitopic` (und `create_dynamic_multitopic` für `DynamicData`) liest die Topics aus dem FROM-Teil des Ausdrucks; `related_topic_names` entfällt. Ein Ausdruck, der keine TopicExpression ist, ein Topic ohne TypeObject oder das nicht besteht, ein Join-Member mit zwei Typen, ein Feld ohne Member und ein Ergebnistyp, der die Felder nicht trägt, sind `BadParameter` (der Grund steht im Ereignis `multitopic.refused`).

### §2.2.2.2.1.9 create_multitopic — Typ vorher registriert

**Spec:** §2.2.2.2.1.9, S. 38 — "The resulting type is specified by the
type_name argument. Prior to creating a MultiTopic the type must have been
registered with the Service."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::the_result_type_must_hold_the_fields_of_the_select_list`

**Status:** `done` — `type_name` ist ein mit `register_type` registrierter Typ, oder `T` hat ein `TypeObject` unter diesem Namen; sonst `BadParameter`.

### §2.2.2.2.1.10 delete_multitopic — löschen

**Spec:** §2.2.2.2.1.10, S. 38 — "This operation deletes a MultiTopic."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_multitopic`;
`crates/dcps/src/topic.rs` (`DescriptionLife`)

**Tests:** `crates/dcps/tests/delete_description.rs::a_deleted_multitopic_releases_its_related_topics_and_answers_already_deleted`

**Status:** `done` — das Löschen gilt für jedes Handle: das MultiTopic hält seine Topics nicht mehr, ein zweites Löschen und `set_expression_parameters` liefern `AlreadyDeleted`.

### §2.2.2.2.1.10 delete_multitopic — PRECONDITION_NOT_MET bei DataReadern

**Spec:** §2.2.2.2.1.10, S. 38 — "If the delete_multitopic operation is called on
a MultiTopic with existing DataReader objects attached to it, it will return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_multitopic_with_a_reader_is_not_deleted_and_its_topic_readers_end_with_it`

**Status:** `done` — solange ein DataReader auf dem MultiTopic besteht, liefert das Löschen `PreconditionNotMet`; mit dem Löschen des Readers (auch durch das Ende seines letzten Handles) ist es erlaubt.

### §2.2.2.2.1.10 delete_multitopic — anderer DomainParticipant

**Spec:** §2.2.2.2.1.10, S. 38 — "If delete_multitopic is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_multitopic`

**Tests:**
`crates/dcps/tests/multitopic.rs::a_multitopic_of_a_foreign_participant_is_not_deleted`

**Status:** `done` — `PreconditionNotMet`, ohne Wirkung.

### §2.2.2.2.1.11 find_topic — bestehendes oder künftiges Topic

**Spec:** §2.2.2.2.1.11, S. 38 — "The operation find_topic gives access to an
existing (or ready to exist) enabled Topic, based on its name … If a Topic of
the same name already exists, it gives access to it, otherwise it waits (blocks
the caller) until another mechanism creates it (or the specified timeout
occurs)."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::find_topic`, `find_enabled_local_topic`, `find_topic_in_sedp`); `crates/dcps/src/topic.rs` (`TopicDescriptionHandle::topic`); `crates/zerodds-c-api/src/participant_ffi.rs` (`zerodds_dp_find_topic_w_timeout`)

**Tests:** `crates/dcps/tests/find_topic.rs::find_topic_gives_a_topic_that_writers_and_readers_are_created_from`, `crates/dcps/tests/find_topic.rs::find_topic_waits_for_a_topic_another_thread_creates`, `crates/dcps/src/participant.rs::tests::find_topic_returns_immediately_for_local`, `crates/dcps/src/participant.rs::tests::find_topic_resolves_after_sedp_publication`, `crates/dcps/src/participant.rs::tests::find_topic_resolves_via_sedp_subscription`, `crates/zerodds-c-api/tests/find_topic_timeout.rs::find_topic_waits_for_a_topic_and_times_out_without_one`

**Status:** done — ein lokales, enabled Topic wird sofort gefunden, eines, das ein anderer Thread anlegt, beendet das Warten, ein entferntes kommt über SEDP. Das Ergebnis ist ein `TopicDescriptionHandle`; `topic::<T>()` liefert das `Topic<T>`, aus dem DataWriter und DataReader entstehen (ein entferntes Topic wird lokal mit dem Default-Topic-QoS angelegt).

### §2.2.2.2.1.11 find_topic — Rückgabe eines Topic

**Spec:** §2.2.2.2.1.11, S. 38 — Rückgabetyp `Topic`; "the Topic is a local
object that acts as a ‘proxy’ to designate the global concept of topic."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescriptionHandle::topic`)

**Tests:** `crates/dcps/tests/find_topic.rs::find_topic_gives_a_topic_that_writers_and_readers_are_created_from`

**Status:** done — siehe das Item darüber: der Handle benennt das Topic, `topic::<T>()` ist das Topic. Den Typ `T` nennt der Aufrufer, weil der Handle untypisiert ist.

### §2.2.2.2.1.11 find_topic — Timeout

**Spec:** §2.2.2.2.1.11, S. 38 — "If the operation times-out, a ‘nil’ value (as
specified by the platform) is returned."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::find_topic`)

**Tests:** `crates/dcps/src/participant.rs::tests::find_topic_times_out_when_unknown`

**Status:** done — `Err(DdsError::Timeout)`.

### §2.2.2.2.1.11 find_topic — mit delete_topic freigeben, nur der lokale Stellvertreter

**Spec:** §2.2.2.2.1.11, S. 38 — "A Topic obtained by means of find_topic, must
also be deleted by means of delete_topic so that the local resources can be
released … the delete_topic operation deletes only the local proxy."

**Repo:** `crates/dcps/src/participant.rs` (`find_enabled_local_topic`, `delete_topic`)

**Tests:** `crates/dcps/tests/find_topic.rs::a_topic_obtained_by_find_topic_is_deleted_as_often`

**Status:** done — `find_topic` zählt als Erhalten des lokalen Topic; `delete_topic` auf dem `Topic<T>` des Handles löscht nur den lokalen Stellvertreter, und das Topic geht mit dem letzten von so vielen `delete_topic` wie es erhalten wurde. Die C-ABI (`zerodds_dp_find_topic_w_timeout`) gibt denselben Zeiger zurück, und ein `zerodds_dp_delete_topic` löscht ihn: dort wird nicht gezählt.

### §2.2.2.2.1.12 lookup_topicdescription — lokales Topic finden

**Spec:** §2.2.2.2.1.12, S. 38–39 — "The operation lookup_topicdescription gives
access to an existing locally-created TopicDescription, based on its name … If a
TopicDescription of the same name already exists, it gives access to it,
otherwise it returns a ‘nil’ value. The operation never blocks … Unlike
find_topic, the operation lookup_topicdescription searches only among the
locally created topics."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::lookup_topicdescription`,
`DomainParticipant::lookup_topicdescription_at` mit `index` über die Topics eines Namens)

**Tests:** `crates/dcps/src/participant.rs::tests::lookup_topicdescription_returns_local_topics`,
`crates/dcps/src/participant.rs::tests::lookup_topicdescription_none_for_unknown`,
`crates/dcps/tests/topics_of_one_name.rs::lookup_iterates_over_the_topics_of_a_name`

**Status:** done — Rückgabe `Option<TopicDescriptionHandle>`; die C-ABI hat keinen `index`.

### §2.2.2.2.1.12 lookup_topicdescription — ContentFilteredTopic und MultiTopic

**Spec:** §2.2.2.2.1.12, S. 39 — "The operation lookup_topicdescription may be
used to locate any locally-created Topic, ContentFilteredTopic, and MultiTopic
object."

**Repo:** `crates/dcps/src/participant.rs` (`lookup_at`, `register_description`); `crates/dcps/src/topic.rs` (`DescriptionEntry`, `TopicDescriptionHandle::content_filtered_topic`, `::multitopic`)

**Tests:** `crates/dcps/tests/find_topic.rs::lookup_finds_a_content_filtered_topic_and_not_a_deleted_one`, `crates/dcps/tests/find_topic.rs::lookup_does_not_find_a_description_without_a_handle`, `crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done — ContentFilteredTopics und MultiTopics stehen (schwach) beim Participant und werden nach den Topics des Namens gefunden; `kind()` unterscheidet sie, `content_filtered_topic::<T>()` und `multitopic::<T>()` geben einen weiteren Handle darauf. Eines ohne Handle oder gelöschtes wird nicht gefunden. Die C-ABI findet nur Topics.

### §2.2.2.2.1.12 lookup_topicdescription — Löschen und spätere Suche

**Spec:** §2.2.2.2.1.12, S. 39 — "It is still possible to delete the
TopicDescription returned by lookup_topicdescription, provided it has no readers
or writers, but then it is really deleted and subsequent lookups will fail."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic_by_name`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_by_name_follows_the_same_rules`,
`crates/dcps/src/participant.rs::tests::delete_topic_removes_the_entry_and_its_handle`

**Status:** done — das Topic hinter dem Namen wird mit
`delete_topic_by_name` gelöscht; danach liefert `lookup_topicdescription`
`None`.

### §2.2.2.2.1.13 get_builtin_subscriber

**Spec:** §2.2.2.2.1.13, S. 39 — "This operation allows access to the built-in
Subscriber. Each DomainParticipant contains several built-in Topic objects as
well as corresponding DataReader objects to access them. All these DataReader
objects belong to a single built-in Subscriber."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_builtin_subscriber`);
`crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::get_builtin_subscriber_returns_subscriber`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::the_builtin_subscriber_is_a_subscriber`

**Status:** done — siehe §2.2.5 „Built-in-Subscriber als Subscriber“.

### §2.2.2.2.1.14 ignore_participant — Built-in-Topics und Zuordnung

**Spec:** §2.2.2.2.1.14, S. 39 — "From that point onwards the Service will
locally behave as if the remote participant did not exist. This means it will
ignore any Topic, publication, or subscription that originates on that domain
participant."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_participant`, `IgnoreFilter`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::disconnect_ignored`, die
Zuordnungsstellen mit `IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_drops_ignored_participant_beacon`,
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_publication`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — die Ankündigungen des Participant und seiner Endpoints
erreichen die Built-in-Reader nicht mehr; bestehende Zuordnungen zu seinen
Endpoints werden getrennt, neue verweigert.

### §2.2.2.2.1.14 ignore_participant — keine Daten mehr

**Spec:** §2.2.2.2.1.14, S. 39 — "the Service will locally behave as if the
remote participant did not exist."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — ein beim Aufruf zugeordneter DataWriter des Participant
wird getrennt; danach geschriebene Samples kommen nicht an.

### §2.2.2.2.1.14 ignore_participant — Handle aus DCPSParticipant

**Spec:** §2.2.2.2.1.14, S. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples available for the built-in
DataReader to the “DCPSParticipant” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`), `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — der `instance_handle` im SampleInfo des
DCPSParticipant-Readers ist `InstanceHandle::from_guid` des Participant-GUID,
derselbe wie in `get_discovered_participants` und im Filter.

### §2.2.2.2.1.14 ignore_participant — nicht umkehrbar

**Spec:** §2.2.2.2.1.14, S. 39 — "The ignore_participant operation is not
required to be reversible. The Service offers no means to reverse it."

**Repo:** `crates/dcps/src/participant.rs` (`IgnoreFilterInner`: nur
Einfügen)

**Tests:** `crates/dcps/src/participant.rs::tests::ignore_is_monotonic_and_idempotent`,
`crates/dcps/src/participant.rs::tests::ignore_participant_records_handle`

**Status:** done — `OUT_OF_RESOURCES` liefert die Operation nicht; die Liste
ist unbegrenzt.

### §2.2.2.2.1.15 ignore_topic — Built-in-Topic

**Spec:** §2.2.2.2.1.15, S. 39 — "This operation allows an application to
instruct the Service to locally ignore a Topic … The Topic to ignore is
identified by the handle argument. This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSTopic” topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::ignore_topic`);
`crates/dcps/src/runtime.rs` (`push_sedp_events_to_builtin_readers`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_drops_the_publications_of_an_ignored_topic`,
`crates/dcps/src/participant.rs::tests::get_discovered_topic_data_filters_ignored`,
`crates/dcps/src/participant.rs::tests::ignore_topic_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — ein ignoriertes Topic erreicht den DCPSTopic-Reader und
`get_discovered_topics` nicht mehr; Publikationen und Abonnements des Topics
werden getrennt und nicht mehr zugeordnet.

### §2.2.2.2.1.15 ignore_topic — Publikationen und Abonnements des Topics

**Spec:** §2.2.2.2.1.15, S. 39 — "This means it will locally ignore any
publication or subscription to the Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::disconnect_ignored`, die
Zuordnungsstellen mit `IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — `IgnoreFilter::ignores_endpoint` fragt an den
Zuordnungsstellen auch das Topic ab (Handle aus dem synthetischen Schlüssel des
DCPSTopic); ein zugeordneter Writer des Topics wird getrennt.

### §2.2.2.2.1.16 ignore_publication

**Spec:** §2.2.2.2.1.16, S. 40 — "This operation allows an application to
instruct the Service to locally ignore a remote publication … After this call,
any data written related to that publication will be ignored … This handle is
the one that appears in the SampleInfo retrieved when reading the data-samples
from the built-in DataReader to the “DCPSPublication” topic."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_publication`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::disconnect_ignored`, die Zuordnungsstellen mit
`IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_publication`,
`crates/dcps/src/participant.rs::tests::ignore_publication_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — die Publikation erreicht den DCPSPublication-Reader nicht
mehr, eine bestehende Zuordnung wird getrennt, eine neue verweigert.

### §2.2.2.2.1.16 ignore_publication — Daten einer zugeordneten Publikation

**Spec:** §2.2.2.2.1.16, S. 40 — "After this call, any data written related to
that publication will be ignored."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — nach dem Aufruf geschriebene Samples der Publikation kommen
nicht an.

### §2.2.2.2.1.17 ignore_subscription

**Spec:** §2.2.2.2.1.17, S. 40 — "This operation allows an application to
instruct the Service to locally ignore a remote subscription … After this call,
any data received related to that subscription will be ignored."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_subscription`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::disconnect_ignored`, die Zuordnungsstellen mit
`IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_subscription`,
`crates/dcps/src/participant.rs::tests::ignore_subscription_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — wie bei `ignore_publication`.

### §2.2.2.2.1.17 ignore_subscription — zugeordnetes Abonnement

**Spec:** §2.2.2.2.1.17, S. 40 — "After this call, any data received related to
that subscription will be ignored."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — ein zugeordneter entfernter DataReader wird getrennt; die
Writes danach erreichen ihn nicht.

### §2.2.2.2.1.18 delete_contained_entities — rekursiv löschen

**Spec:** §2.2.2.2.1.18, S. 40 — "This operation deletes all the entities that
were created by means of the “create” operations on the DomainParticipant. That
is, it deletes all contained Publisher, Subscriber, Topic, ContentFilteredTopic,
and MultiTopic … delete_contained_entities on the DomainParticipant will end up
deleting all the entities recursively contained in the DomainParticipant, that
is also the DataWriter, DataReader, as well as the QueryCondition and
ReadCondition objects belonging to the contained DataReaders."

**Repo:** `crates/dcps/src/participant.rs` (`delete_contained_entities`, `delete_all_descriptions`), `crates/dcps/src/topic.rs` (`DescriptionEntry::delete_handle`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::participant_delete_contained_entities_deletes_everything_it_created`,
`crates/dcps/src/participant.rs::tests::delete_contained_entities_clears_topics_and_groups`,
`crates/dcps/tests/reader_conditions.rs::delete_contained_entities_deletes_every_condition_the_reader_made`,
`crates/dcps/tests/entity_scoping.rs::delete_contained_entities_deletes_a_content_filtered_topic`

**Status:** done — Publisher, Subscriber, DataWriter, DataReader, ihre Conditions, die Topics sowie ContentFilteredTopic und MultiTopic werden gelöscht (die Beschreibungen vor den Topics, die sie benutzen).

### §2.2.2.2.1.18 delete_contained_entities — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.18, S. 40 — "The operation will return
PRECONDITION_NOT_MET if the any of the contained entities is in a state where
it cannot be deleted."

**Repo:** `crates/dcps/src/participant.rs` (`delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET, ohne dass etwas gelöscht wird, solange ein GROUP-Publisher in einem kohärenten Satz oder ein Subscriber in einem Gruppenzugriff ist.

### §2.2.2.2.1.19 assert_liveliness

**Spec:** §2.2.2.2.1.19, S. 40 — "This operation manually asserts the liveliness
of the DomainParticipant … This operation needs to only be used if the
DomainParticipant contains DataWriter entities with the LIVELINESS set to
MANUAL_BY_PARTICIPANT and it only affects the liveliness of those DataWriter
entities. Otherwise, it has no effect."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_liveliness`),
erreichbar über `DomainParticipant::runtime`

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`,
`crates/dcps/src/runtime.rs::tests::assert_liveliness_enqueues_wlp_pulse_without_panic`,
`crates/dcps/tests/entity_scoping.rs::assert_liveliness_keeps_a_manual_by_participant_writer_alive`

**Status:** done — `DomainParticipant::assert_liveliness` hält die Writer mit MANUAL_BY_PARTICIPANT am Leben; ohne Aufruf läuft die Lease ab (LIVELINESS_LOST).

### §2.2.2.2.1.19 assert_liveliness — Operation am DomainParticipant

**Spec:** §2.2.2.2.1.19, S. 34 und 40 — Operation `assert_liveliness` der
Klasse DomainParticipant, Rückgabe `ReturnCode_t`.

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::assert_liveliness`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::assert_liveliness_keeps_a_manual_by_participant_writer_alive`, `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — die Operation besteht am `DomainParticipant` (NOT_ENABLED vor `enable`).

### §2.2.2.2.1.20 set_default_publisher_qos

**Spec:** §2.2.2.2.1.20, S. 41 — "This operation sets a default value of the
Publisher QoS policies which will be used for newly created Publisher entities
in the case where the QoS policies are defaulted in the create_publisher
operation … if they are not, the operation will have no effect and return
INCONSISTENT_POLICY … The special value PUBLISHER_QOS_DEFAULT may be passed …
to indicate that the default QoS should be reset back to the initial values".

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_publisher_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — gespeichert am Participant; `PUBLISHER_QOS_DEFAULT` setzt
auf den Spec-Default zurück. Eine PublisherQos kann nicht inkonsistent sein
(§2.2.3 nennt für PRESENTATION, PARTITION, GROUP_DATA und ENTITY_FACTORY keine
Regel).

### §2.2.2.2.1.21 get_default_publisher_qos

**Spec:** §2.2.2.2.1.21, S. 41 — "The values retrieved get_default_publisher_qos
will match the set of values specified on the last successful call to
set_default_publisher_qos, or else, if the call was never made, the default
values listed in the QoS table in 2.2.3".

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_publisher_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — der zuletzt gesetzte Wert, ohne Aufruf der Spec-Default.

### §2.2.2.2.1.22 set_default_subscriber_qos

**Spec:** §2.2.2.2.1.22, S. 41 — wie set_default_publisher_qos für den
Subscriber, mit INCONSISTENT_POLICY und SUBSCRIBER_QOS_DEFAULT.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_subscriber_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — wie `set_default_publisher_qos`.

### §2.2.2.2.1.23 get_default_subscriber_qos

**Spec:** §2.2.2.2.1.23, S. 41 — wie get_default_publisher_qos für den
Subscriber.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_subscriber_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — wie `get_default_publisher_qos`.

### §2.2.2.2.1.24 set_default_topic_qos

**Spec:** §2.2.2.2.1.24, S. 41 — wie set_default_publisher_qos für das Topic,
mit INCONSISTENT_POLICY und TOPIC_QOS_DEFAULT.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_topic_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — eine inkonsistente TopicQos (HISTORY gegen RESOURCE_LIMITS)
ist INCONSISTENT_POLICY und ändert den Default nicht.

### §2.2.2.2.1.25 get_default_topic_qos

**Spec:** §2.2.2.2.1.25, S. 42 — wie get_default_publisher_qos für das Topic.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_topic_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — wie `get_default_publisher_qos`.

### §2.2.2.2.1.26 get_domain_id

**Spec:** §2.2.2.2.1.26, S. 42 — "This operation retrieves the domain_id used to
create the DomainParticipant."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::domain_id`)

**Tests:** `crates/dcps/src/participant.rs::tests::participant_created_with_domain_id`,
`crates/dcps/src/factory.rs::tests::factory_creates_participant_with_correct_domain_id`

**Status:** done

### §2.2.2.2.1.27 get_discovered_participants

**Spec:** §2.2.2.2.1.27, S. 42 — "This operation retrieves the list of
DomainParticipants that have been discovered in the domain and that the
application has not indicated should be “ignored” … The operation may fail if
the infrastructure does not locally maintain the connectivity information. In
this case the operation will return UNSUPPORTED."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_participants`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_participants_lists_after_spdp_inject`,
`crates/dcps/src/participant.rs::tests::get_discovered_participants_offline_is_empty`

**Status:** done — ohne Laufzeit liefert die Operation eine leere Liste statt
UNSUPPORTED.

### §2.2.2.2.1.28 get_discovered_participant_data — Daten

**Spec:** §2.2.2.2.1.28, S. 42 — "This operation retrieves information on a
DomainParticipant that has been discovered on the network."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_participant_data`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_participants_lists_after_spdp_inject`

**Status:** done

### §2.2.2.2.1.28 get_discovered_participant_data — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.28, S. 42 — "The participant_handle must correspond to such
a DomainParticipant. Otherwise, the operation will fail and return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::a_handle_that_is_no_discovered_entity_is_precondition_not_met`

**Status:** done — ein unbekannter oder ignorierter Handle liefert PRECONDITION_NOT_MET.

### §2.2.2.2.1.29 get_discovered_topics

**Spec:** §2.2.2.2.1.29, S. 42 — "This operation retrieves the list of Topics
that have been discovered in the domain and that the application has not
indicated should be “ignored” by means of the DomainParticipant ignore_topic
operation."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_topics`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_topics_lists_unique_handles_for_pub_and_sub`,
`crates/dcps/src/participant.rs::tests::get_discovered_topic_data_filters_ignored`,
`crates/dcps/src/participant.rs::tests::get_discovered_topics_offline_is_empty`

**Status:** done

### §2.2.2.2.1.30 get_discovered_topic_data — Daten

**Spec:** §2.2.2.2.1.30, S. 42 — "This operation retrieves information on a
Topic that has been discovered on the network."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_topic_data`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_topics_lists_unique_handles_for_pub_and_sub`

**Status:** done

### §2.2.2.2.1.30 get_discovered_topic_data — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.30, S. 42 — "The topic_handle must correspond to such a
topic. Otherwise, the operation will fail and return PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::a_handle_that_is_no_discovered_entity_is_precondition_not_met`

**Status:** done — ein unbekannter oder ignorierter Handle liefert PRECONDITION_NOT_MET.

### §2.2.2.2.1.31 contains_entity

**Spec:** §2.2.2.2.1.31, S. 43 — "This operation checks whether or not the given
a_handle represents an Entity that was created from the DomainParticipant. The
containment applies recursively."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::contains_entity`);
`crates/dcps/src/publisher.rs` (`Publisher::contains_writer`),
`crates/dcps/src/subscriber.rs` (`Subscriber::contains_reader`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_self_handle`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datareader`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_does_not_find_foreign_datawriter`,
`crates/dcps/src/participant.rs::tests::contains_entity_returns_false_for_topic_after_delete`

**Status:** done

### §2.2.2.2.1.32 get_current_time

**Spec:** §2.2.2.2.1.32, S. 43 — "This operation returns the current value of
the time that the service uses to time-stamp data-writes and to set the
reception-timestamp for the data-updates it receives."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::get_current_time`,
`crates/zerodds-c-api/src/participant_ffi.rs::zerodds_dp_get_current_time`

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::get_current_time_is_the_clock_writes_are_stamped_with`,
`crates/zerodds-c-api/src/participant_ffi.rs::tests::get_current_time_returns_nonzero`

**Status:** `done` — die Uhr, mit der der Participant Writes stempelt
(`RuntimeConfig::time_source`), offline die Systemuhr; im C-API ebenso.

### §2.2.2.2.1.32 get_current_time — Uhr des Participant

**Spec:** §2.2.2.2.1.32, S. 43 — die Zeit, "that the service uses to time-stamp
data-writes".

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::get_current_time`

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::get_current_time_is_the_clock_writes_are_stamped_with`

**Status:** `done` — liest `rt.config.time_source.now()`; der Test vergleicht
mit dem Quellzeitstempel eines Write unter einer gesetzten Zeitquelle.

### §2.2.2.2.2 DomainParticipantFactory — Singleton, kein Entity

**Spec:** §2.2.2.2.2, S. 43 — "The sole purpose of this class is to allow the
creation and destruction of DomainParticipant objects. DomainParticipantFactory
itself has no factory. It is a pre-existing singleton object".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done

### §2.2.2.2.2.1 create_participant — erzeugen

**Spec:** §2.2.2.2.2.1, S. 44 — "This operation creates a new DomainParticipant
object. The DomainParticipant signifies that the calling application intends to
join the Domain identified by the domain_id argument … In case of failure, the
operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::create_participant`,
`DomainParticipantFactory::create_participant_with_config`,
`ParticipantBuilder::build`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_creates_participant_with_correct_domain_id`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`

**Status:** done — ein Fehler (Sockets, Konfiguration, Security) ist
`Err(DdsError)`. Für `DomainParticipantQos` nennt §2.2.3 keine inkonsistente
Kombination.

### §2.2.2.2.2.1 create_participant — Listener und Maske

**Spec:** §2.2.2.2.2.1, S. 43 — Parameter `a_listener:
DomainParticipantListener`, `mask: StatusKind []`.

**Repo:** `create_participant_with_listener`

**Tests:** `crates/dcps/tests/nil_listener.rs::create_participant_takes_a_listener_and_a_mask`

**Status:** done — `create_participant_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.2.2.1 create_participant — PARTICIPANT_QOS_DEFAULT

**Spec:** §2.2.2.2.2.1, S. 44 — "The special value PARTICIPANT_QOS_DEFAULT can
be used to indicate that the DomainParticipant should be created with the
default DomainParticipant QoS set in the factory."

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::create_participant`,
`::create_participant_with_config`, `::create_participant_offline`,
`ParticipantBuilder`

**Tests:** `crates/dcps/tests/default_qos.rs::the_participant_factory_default`

**Status:** `done` — `PARTICIPANT_QOS_DEFAULT` nimmt
`get_default_participant_qos`; `ParticipantBuilder` ohne `with_qos` ebenso.

### §2.2.2.2.2.2 delete_participant — löschen

**Spec:** §2.2.2.2.2.2, S. 44 — "This operation deletes an existing
DomainParticipant."

**Repo:** `crates/dcps/src/factory.rs`
(`DomainParticipantFactory::delete_participant`)

**Tests:**
`crates/dcps/src/factory.rs::tests::delete_participant_removes_from_registry`,
`crates/dcps/src/factory.rs::tests::delete_participant_unknown_returns_precondition_not_met`,
`crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`

**Status:** `done` — der Participant geht aus der Factory, wird als gelöscht
markiert und seine Laufzeit beendet.

### §2.2.2.2.2.2 delete_participant — Participant danach gelöscht

**Spec:** §2.2.2.2.2.2, S. 44 mit §2.2.1.1, S. 17 — nach dem Löschen "the
operation should fail and return ALREADY_DELETED".

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::delete_participant`
(`mark_deleted`, `DcpsRuntime::shutdown`)

**Tests:** `crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`,
`crates/dcps/src/factory.rs::tests::a_deleted_participant_creates_no_publisher_or_subscriber`

**Status:** `done` — Operationen mit `Result` liefern `AlreadyDeleted`; `try_create_publisher` und `try_create_subscriber` ebenso, `create_publisher` und `create_subscriber` eine gelöschte Entity.

### §2.2.2.2.2.2 delete_participant — PRECONDITION_NOT_MET mit Entities

**Spec:** §2.2.2.2.2.2, S. 44 — "This operation can only be invoked if all
domain entities belonging to the participant have already been deleted.
Otherwise the error PRECONDITION_NOT_MET is returned."

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::delete_participant`
(`has_contained_entities`)

**Tests:**
`crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`

**Status:** `done` — mit Publisher, Subscriber oder Topic `PreconditionNotMet`,
ohne Wirkung.

### §2.2.2.2.2.3 get_instance

**Spec:** §2.2.2.2.2.3, S. 44 — "This operation returns the
DomainParticipantFactory singleton. The operation is idempotent".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::instance`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done

### §2.2.2.2.2.3 get_instance — TheParticipantFactory

**Spec:** §2.2.2.2.2.3, S. 44 — "The pre-defined value TheParticipantFactory can
also be used as an alias for the singleton factory returned by the operation
get_instance."

**Repo:** `crates/dcps/src/factory.rs::THE_PARTICIPANT_FACTORY`

**Tests:**
`crates/dcps/tests/default_qos.rs::the_participant_factory_is_the_instance`

**Status:** `done` — `THE_PARTICIPANT_FACTORY` ist die Instanz von
`DomainParticipantFactory::instance()`.

### §2.2.2.2.2.4 lookup_participant

**Spec:** §2.2.2.2.2.4, S. 44 — "This operation retrieves a previously created
DomainParticipant belonging to specified domain_id. If no such
DomainParticipant exists, the operation will return a ‘nil’ value. If multiple
DomainParticipant entities belonging to that domain_id exist, then the operation
will return one of them."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::lookup_participant`)

**Tests:** `crates/dcps/src/factory.rs::tests::lookup_participant_finds_registered_offline_participant`,
`crates/dcps/src/factory.rs::tests::lookup_participant_returns_none_for_unknown_domain`,
`crates/dcps/src/factory.rs::tests::dropping_participant_releases_it_from_the_factory`

**Status:** done

### §2.2.2.2.2.5 set_default_participant_qos

**Spec:** §2.2.2.2.2.5, S. 44 — "This operation sets a default value of the
DomainParticipant QoS policies which will be used for newly created
DomainParticipant entities in the case where the QoS policies are defaulted in
the create_participant operation … if they are not [self consistent], the
operation will have no effect and return INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::set_default_participant_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::default_participant_qos_roundtrips`

**Status:** done — der Wert wird gespeichert und gelesen; dass
`create_participant` ihn nicht verwendet, ist unter §2.2.2.2.2.1
„PARTICIPANT_QOS_DEFAULT“ geführt. Für `DomainParticipantQos` gibt es keine
inkonsistente Kombination.

### §2.2.2.2.2.6 get_default_participant_qos

**Spec:** §2.2.2.2.2.6, S. 45 — "The values retrieved get_default_participant_qos
will match the set of values specified on the last successful call to
set_default_participant_qos, or else, if the call was never made, the default
values listed in the QoS table in 2.2.3".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::get_default_participant_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::default_participant_qos_roundtrips`

**Status:** done

### §2.2.2.2.2.7 set_qos (Factory)

**Spec:** §2.2.2.2.2.7, S. 45 — "This operation sets the value of the
DomainParticipantFactory QoS policies … Note that despite having QoS, the
DomainParticipantFactory is not an Entity … if they are not [self consistent],
the operation will have no effect and return INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::set_qos`,
`DomainParticipantFactoryQos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_set_get_qos_roundtrip`

**Status:** done — die Factory-QoS ist ENTITY_FACTORY; sie kann nicht
inkonsistent sein.

### §2.2.2.2.2.8 get_qos (Factory)

**Spec:** §2.2.2.2.2.8, S. 45 — "This operation returns the value of the
DomainParticipantFactory QoS policies."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::get_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`,
`crates/dcps/src/factory.rs::tests::factory_set_get_qos_roundtrip`

**Status:** done

### §2.2.2.2.3 DomainParticipantListener — alle Callbacks

**Spec:** §2.2.2.2.3, S. 45–46 — "The DomainParticipantListener interface extends
all other Listener interfaces and has no additional operation beyond the ones
defined by the more general listeners." Operationen: on_inconsistent_topic,
on_liveliness_lost, on_offered_deadline_missed, on_offered_incompatible_qos,
on_data_on_readers, on_sample_lost, on_data_available, on_sample_rejected,
on_liveliness_changed, on_requested_deadline_missed,
on_requested_incompatible_qos, on_publication_matched,
on_subscription_matched.

**Repo:** `crates/dcps/src/listener.rs` (`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — alle dreizehn Callbacks, je mit leerer Vorgabe. Die
Parameter (Handle statt Entity) sind unter §2.2.4.3 geführt.

### §2.2.2.2.3 DomainParticipantListener — Listener der letzten Instanz

**Spec:** §2.2.2.2.3, S. 46 — "When a relevant status change occurs, the DCPS
Service will first attempt to notify the listener attached to the concerned
DomainEntity if one is installed. Otherwise, the DCPS Service will notify the
Listener attached to the DomainParticipant."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (Kette Reader → Subscriber →
Participant und Writer → Publisher → Participant)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::data_available_bubbles_all_the_way_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_bubbles_to_participant`,
`crates/dcps/tests/listener_integration.rs::push_raw_bubbles_to_participant_when_others_have_no_listener`

**Status:** done

---

## §2.2.2.3 Topic-Definition Module

### §2.2.2.3 Klassen des Topic-Definition Module

**Spec:** §2.2.2.3, S. 47 — TopicDescription, Topic, ContentFilteredTopic,
MultiTopic, TopicListener, TypeSupport.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Klassen sind unten einzeln
geführt.

### §2.2.2.3.1 TopicDescription — Basis von Topic, ContentFilteredTopic, MultiTopic

**Spec:** §2.2.2.3.1, S. 47–48 — "This class is an abstract class. It is the
base class for Topic, ContentFilteredTopic, and MultiTopic … Its attribute
type_name defines a unique resulting type for the publication or the
subscription and therefore creates an implicit association with a TypeSupport.
TopicDescription has also a name that allows it to be retrieved locally."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription`, implementiert für
`Topic<T>`, `ContentFilteredTopic<T>`, `MultiTopic<T>`,
`TopicDescriptionHandle`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`,
`crates/dcps/src/topic.rs::tests::cft_compiles_and_evaluates_filter`,
`crates/dcps/src/topic.rs::tests::topic_description_trait_is_object_safe`

**Status:** done

### §2.2.2.3.1.1 get_participant

**Spec:** §2.2.2.3.1.1, S. 48 — "This operation returns the DomainParticipant to
which the TopicDescription belongs."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_participant`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_description_get_participant_returns_owning_participant`

**Status:** done

### §2.2.2.3.1.2 type_name

**Spec:** §2.2.2.3.1.2, S. 48 — "The type_name used to create the
TopicDescription."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_type_name`,
`Topic::type_name`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done

### §2.2.2.3.1.3 name

**Spec:** §2.2.2.3.1.3, S. 48 — "The name used to create the TopicDescription."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_name`,
`Topic::name`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/src/topic.rs::tests::cft_compiles_and_evaluates_filter`

**Status:** done

### §2.2.2.3.2 Topic — Name eindeutig im Domain

**Spec:** §2.2.2.3.2, S. 49 — "A Topic is identified by its name, which must be
unique in the whole Domain. In addition (by virtue of extending
TopicDescription) it fully specifies the type of the data that can be
communicated when publishing or subscribing to the Topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic_with_support`:
gleicher Name mit anderer QoS → `InconsistentPolicy`, anderer Typ bei gleicher QoS →
weiteres Topic);
`crates/dcps/src/topic.rs` (`inconsistent_topic_total`: Zähler aus
Discovery)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_takes_a_second_type_and_rejects_an_inconsistent_qos`,
`crates/dcps/tests/topics_of_one_name.rs::a_second_type_under_one_name_is_a_second_topic`,
`crates/dcps/tests/topics_of_one_name.rs::an_inconsistent_qos_is_refused_and_counted`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`

**Status:** done — ein weiterer Typ unter einem benutzten Namen ist ein weiteres
Topic, wenn die QoS gleich ist; bei abweichender QoS wird abgewiesen und
INCONSISTENT_TOPIC gezählt. Domainweit meldet ein Widerspruch INCONSISTENT_TOPIC
(§2.2.4.1).

### §2.2.2.3.2 Topic — einzige TopicDescription für DataWriter

**Spec:** §2.2.2.3.2, S. 49 — "Topic is the only TopicDescription that can be
used for publications and therefore associated to a DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter` nimmt
`&Topic<T>`)

**Tests:** `crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`

**Status:** done — durch die Signatur.

### §2.2.2.3.2 Topic — NOT_ENABLED

**Spec:** §2.2.2.3.2, S. 49 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable and
get_status_condition may return the value NOT_ENABLED."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`, `crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — der Test deckt es ab.

### §2.2.2.3.2.1 get_inconsistent_topic_status

**Spec:** §2.2.2.3.2.1, S. 49 — "This method allows the application to retrieve
the INCONSISTENT_TOPIC status of the Topic."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`

**Status:** done — der Test deckt es ab.

### §2.2.2.3.3 ContentFilteredTopic — Filterausdruck und Parameter

**Spec:** §2.2.2.3.3, S. 49 — "The filter_expression attribute is a string that
specifies the criteria to select the data samples of interest. It is similar to
the WHERE part of an SQL clause. The expression_parameters attribute is a
sequence of strings that give values to the ‘parameters’ (i.e., "%n" tokens) in
the filter_expression. The number of supplied parameters must fit with the
requested values in the filter_expression".

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::new`,
`ContentFilteredTopic::evaluate`); `crates/sql-filter/src/parser.rs`

**Tests:** `crates/dcps/src/topic.rs::tests::cft_param_index_out_of_range_rejected`,
`crates/dcps/src/topic.rs::tests::cft_filter_with_string_param`,
`crates/dcps/src/topic.rs::tests::cft_invalid_expression_rejected`

**Status:** done — zu wenige Parameter für die verwendeten `%n` werden
abgewiesen; überzählige Parameter werden angenommen.

### §2.2.2.3.3 ContentFilteredTopic — inhaltsbasiertes Abonnement

**Spec:** §2.2.2.3.3, S. 49 — "ContentFilteredTopic describes a more
sophisticated subscription that indicates the subscriber does not want to
necessarily see all values of each instance published under the Topic. Rather,
it wants to see only the values whose contents satisfy certain criteria."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader_with_cft`);
`crates/dcps/src/topic.rs` (`CftEvaluator`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filtered_reader.rs::a_field_the_type_does_not_answer_delivers_nothing`,
`crates/dcps/tests/content_filter.rs::filter_drops_samples_that_return_false`

**Status:** done

### §2.2.2.3.3.1 get_related_topic

**Spec:** §2.2.2.3.3.1, S. 50 — "This operation returns the Topic associated
with the ContentFilteredTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_related_topic`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_get_related_topic`

**Status:** done

### §2.2.2.3.3.2 filter_expression

**Spec:** §2.2.2.3.3.2, S. 50 — "The filter_expression associated with the
ContentFilteredTopic. That is, the expression specified when the
ContentFilteredTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_filter_expression`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`

**Status:** done

### §2.2.2.3.3.3 get_expression_parameters

**Spec:** §2.2.2.3.3.3, S. 50 — "This operation returns the expression_parameters
associated with the ContentFilteredTopic. That is, the parameters specified on
the last successful call to set_expression_parameters, or if
set_expression_parameters was never called, the parameters specified when the
ContentFilteredTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_filter_parameters`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`,
`crates/dcps/src/topic.rs::tests::cft_clone_shares_params`

**Status:** done — die Operation heißt `get_filter_parameters`.

### §2.2.2.3.3.4 set_expression_parameters

**Spec:** §2.2.2.3.3.4, S. 50 — "This operation changes the
expression_parameters associated with the ContentFilteredTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::set_filter_parameters`:
neue Parameter, Neuankündigung je Reader)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`,
`crates/dcps/src/topic.rs::tests::cft_set_filter_parameters_validates_count`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** done — die Operation heißt `set_filter_parameters`.

### §2.2.2.3.4 MultiTopic [optional] — Daten mehrerer Topics kombinieren

**Spec:** §2.2.2.3.4, S. 50–51 — "MultiTopic allows a more sophisticated
subscription that can select and combine data received from multiple topics
into a single resulting type … The subscription_expression … is similar to an
SQL clause where the SELECT part provides the fields to be kept, the FROM part
provides the names of the topics that are searched for those fields, and the
WHERE clause gives the content filter. The Topics combined may have different
types but they are restricted in that the type of the fields used for the
NATURAL JOIN operation must be the same."

**Repo:** `crates/dcps/src/multitopic.rs`;
`crates/dcps/src/topic.rs::MultiTopic`;
`crates/dcps/src/subscriber.rs::Subscriber::create_datareader_with_multitopic`

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`,
`crates/dcps/tests/multitopic.rs::live::projection_alias_where_and_parameter`,
`crates/dcps/tests/multitopic.rs::live::the_join_runs_over_an_inherited_member`,
`crates/dcps/tests/multitopic.rs::inherited_members_are_fields_of_the_structure`,
`crates/dcps/tests/multitopic.rs::live::the_readers_of_the_topics_are_not_visible_to_the_application`

**Status:** `done` — Join der Topics des FROM-Teils über alle Member gleichen Namens (der SQL-Natural-Join, den Annex B.2 „NATURAL JOIN“ nennt; ohne gemeinsamen Member das kartesische Produkt), SELECT behält und benennt Felder, WHERE wertet die verbundene Zeile mit den Aliasen aus. Geerbte Member einer Basis-Struktur sind Felder der Struktur. Die Reader der Topics gehören dem Reader des MultiTopic und sind für die Anwendung nicht sichtbar. Modell: `crates/dcps/src/multitopic.rs`.

### §2.2.2.3.4 MultiTopic — Anzahl der Parameter

**Spec:** §2.2.2.3.4, S. 51 — "The number of supplied parameters must fit with
the requested values in the subscription_expression (i.e., the number of %n
tokens)."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::new`,
`MultiTopic::set_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.4 MultiTopic — DataReader werden benachrichtigt

**Spec:** §2.2.2.3.4, S. 51 — "DataReader entities associated with a MultiTopic
are alerted of data modifications by the usual Listener or Condition mechanisms
… whenever modifications occur to the data associated with any of the topics
relevant to the MultiTopic."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`

**Status:** `done` — die Reader der Topics des FROM-Teils melden dem Reader des MultiTopic den Eingang (`on_data_available`, Bedingungen); `DataReader` auf einem MultiTopic: `Subscriber::create_datareader_with_multitopic`.

### §2.2.2.3.4 MultiTopic — zusammengesetzte Instanzen

**Spec:** §2.2.2.3.4, S. 51 — "DataReader entities associated with a MultiTopic
access instances that are “constructed” at the DataReader side from the
instances written by multiple DataWriter entities. The MultiTopic access
instance will begin to exist as soon as all the constituting Topic instances are
in existence."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`

**Status:** `done` — ein Ergebnis entsteht, sobald zu einer Instanz eines Topics Instanzen aller anderen vorhanden sind; eine Bestellung ohne Kunden wartet.

### §2.2.2.3.4 MultiTopic — view_state der zusammengesetzten Instanz

**Spec:** §2.2.2.3.4, S. 51 — "The view_state … of the MultiTopic instance is
NEW if at least one of the constituting instances has view_state = NEW,
otherwise it will be NOT_NEW."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_new_constituent_makes_the_result_instance_new`

**Status:** `done` — NEW, wenn mindestens eine Bestandteil-Instanz NEW ist, sonst NOT_NEW. Eine Bestandteil-Instanz ist NEW ab ihrem ersten Sample und wieder ab dem ersten nach einem Zustand ohne Leben, bis ein Ergebnis-Sample aus ihr gebildet wurde; ein Ergebnis-Sample aus einem NEW-Bestandteil macht die Ergebnis-Instanz für das nächste read oder take wieder NEW (`InstanceTracker::mark_view_new`).

### §2.2.2.3.4 MultiTopic — instance_state der zusammengesetzten Instanz

**Spec:** §2.2.2.3.4, S. 51 — "The instance_state … of the MultiTopic instance
is “ALIVE” if the instance_state of all the constituting Topic instances is
ALIVE. It is “NOT_ALIVE_DISPOSED” if at least one of the constituting Topic
instances is NOT_ALIVE_DISPOSED. Otherwise it is NOT_ALIVE_NO_WRITERS."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_disposed_constituent_disposes_the_result_instance`,
`crates/dcps/tests/multitopic.rs::live::an_unregistered_constituent_leaves_the_result_instance_without_writers`

**Status:** `done` — NOT_ALIVE_DISPOSED, sobald ein Bestandteil verworfen ist, sonst NOT_ALIVE_NO_WRITERS, sobald einer keine Writer hat; die Ergebnis-Instanz erfährt es als Marker.

### §2.2.2.3.4.1 subscription_expression

**Spec:** §2.2.2.3.4.1, S. 51 — "The subscription_expression associated with the
MultiTopic. That is, the expression specified when the MultiTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::get_subscription_expression`)

**Tests:** `crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done

### §2.2.2.3.4.2 get_expression_parameters (MultiTopic)

**Spec:** §2.2.2.3.4.2, S. 51 — "This operation returns the expression_parameters
associated with the MultiTopic. That is, the parameters specified on the last
successful call to set_expression_parameters, or if set_expression_parameters
was never called, the parameters specified when the MultiTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::get_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.4.3 set_expression_parameters (MultiTopic)

**Spec:** §2.2.2.3.4.3, S. 51 — "This operation changes the
expression_parameters associated with the MultiTopic."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::set_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.5 TopicListener — on_inconsistent_topic

**Spec:** §2.2.2.3.5, S. 52 — "Since Topic is a kind of Entity, it has the
ability to have an associated listener. In this case, the associated listener
should be of concrete type TopicListener." Operation `on_inconsistent_topic(the_topic:
Topic, status: InconsistentTopicStatus)`.

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener::on_inconsistent_topic`);
`crates/dcps/src/topic.rs` (`poll_inconsistent_topic`)

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_no_delta_no_listener`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_consumed_at_topic`

**Status:** done — der Callback erhält den Handle des Topics statt des Topics
(siehe §2.2.4.3).

### §2.2.2.3.6 TypeSupport — automatisch erzeugt

**Spec:** §2.2.2.3.6, S. 52 — "It is required that each implementation of the
Service provides an automatic means to generate this type-specific class from a
description of the type (using IDL for example in the OMG IDL mapping)."

**Repo:** `crates/zerodds-build/src/dds_type.rs` (`dds_type_impls`: aus IDL
erzeugte `impl zerodds_dcps::DdsType`); `crates/dcps/src/dds_type.rs`
(`DdsType`)

**Tests:** `crates/zerodds-build/tests/compile.rs::a_struct_becomes_a_type_with_a_codec`,
`crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/zerodds-build/tests/dds_type.rs::a_keyed_type_carries_its_holder_and_the_step_five_branch`

**Status:** done

### §2.2.2.3.6.1 register_type

**Spec:** §2.2.2.3.6, S. 52 — "A TypeSupport must be registered using the
register_type operation on this type-specific class before it can be used to
create Topic objects."; §2.2.2.3.6.1, S. 52 — "This operation allows an
application to communicate to the Service the existence of a data type … This
includes in particular the key definition".

**Repo:** `crates/dcps/src/type_support.rs` (`TypeSupport::register_type`), `crates/dcps/src/participant.rs` (`register_type_support`)

**Tests:** `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`, `crates/dcps/tests/type_support.rs::the_type_support_names_its_type`

**Status:** done — `TypeSupport::<T>::register_type(&participant, Option<&str>)`; Schlüssel und Kodierung stehen im `DdsType`-Impl.

### §2.2.2.3.6.1 register_type — gleicher Name für zwei TypeSupports

**Spec:** §2.2.2.3.6.1, S. 52 — "It is a pre-condition error to use the same
type_name to register two different TypeSupport with the same
DomainParticipant. If an application attempts this, the operation will fail and
return PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/type_support.rs::one_type_name_is_not_two_type_supports`

**Status:** done — derselbe Name für zwei verschiedene TypeSupports am selben Participant liefert PRECONDITION_NOT_MET; an einem anderen Participant ist der Name frei.

### §2.2.2.3.6.1 register_type — mehrfach, anderer Name, nil

**Spec:** §2.2.2.3.6.1, S. 52 — "it is allowed to register the same TypeSupport
multiple times with a DomainParticipant using the same or different values for
the type_name … the second (and subsequent) registrations are ignored but the
operation returns OK. The application may pass nil as the value for the
type_name. In this case the default type-name … will be used."

**Repo:** —

**Tests:** `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`

**Status:** done — dasselbe TypeSupport mehrfach unter gleichem oder anderem Namen wird ignoriert und liefert OK; `None` (nil) verwendet den Standardnamen.

### §2.2.2.3.6.2 get_type_name

**Spec:** §2.2.2.3.6.2, S. 53 — "This operation returns the default name for the
data-type represented by the TypeSupport."

**Repo:** `crates/dcps/src/dds_type.rs` (`DdsType::TYPE_NAME`,
`DdsType::type_name`)

**Tests:** `crates/dcps/src/dds_type.rs::tests::raw_bytes_type_name_is_namespaced`,
`crates/dcps/src/publisher.rs::tests::datawriter_type_name_follows_the_topic_type`

**Status:** done

### §2.2.2.3.7 Abgeleitete Klassen je Anwendungstyp

**Spec:** §2.2.2.3.7, S. 53 — "For each data class defined by the application,
there is a number of specialized classes that are required to facilitate the
type-safe interaction of the application with the Service. It is required that
each implementation of the Service provides an automatic means to generate all
these type-specific classes." (FooTypeSupport, FooDataWriter, FooDataReader)

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`),
`crates/dcps/src/subscriber.rs` (`DataReader<T>`),
`crates/zerodds-build/src/dds_type.rs` (`dds_type_impls`)

**Tests:** `crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`

**Status:** done — die typisierten Klassen sind generische Typen über `T:
DdsType`; erzeugt wird nur der `DdsType`-Impl.

---

## §2.2.2.4 Publication Module

### §2.2.2.4 Klassen des Publication Module

**Spec:** §2.2.2.4, S. 54 — Publisher, DataWriter, PublisherListener,
DataWriterListener.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Klassen sind unten einzeln
geführt.

### §2.2.2.4.1 Publisher — Verbreitung der Publikationen

**Spec:** §2.2.2.4.1, S. 55–56 — "A Publisher is the object responsible for the
actual dissemination of publications … When it is informed of a change to the
data associated with one of its DataWriter objects, it decides when it is
appropriate to actually send the data-update message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Rollenbeschreibung; die Operationen sind
unten einzeln geführt. Die Regel zu NOT_ENABLED (S. 56) steht unter
§2.2.2.1.1.7 „NOT_ENABLED auf DomainParticipant, Publisher, Subscriber, Topic“.

### §2.2.2.4.1.1 set_listener (Publisher)

**Spec:** §2.2.2.4.1.1, S. 56 — "a Publisher can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend PublisherListener."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`

**Status:** done — nachträglich; beim Erzeugen nicht (siehe §2.2.2.2.1.1
„Listener und Maske“).

### §2.2.2.4.1.2 get_listener (Publisher)

**Spec:** §2.2.2.4.1.2, S. 56 — "Retrieves the attached PublisherListener."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.4.1.3 set_qos (Publisher)

**Spec:** §2.2.2.4.1.3, S. 56 — "a Publisher can be given QoS at creation time
or later by using the set_qos operation … Possible error codes returned in
addition to the standard ones: IMMUTABLE_POLICY, INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`

**Status:** done — PRESENTATION nach `enable` → `ImmutablePolicy`; eine neue
PARTITION oder GROUP_DATA erreicht die DataWriter des Publishers.

### §2.2.2.4.1.4 get_qos (Publisher)

**Spec:** §2.2.2.4.1.4, S. 56 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::get_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`

**Status:** done

### §2.2.2.4.1.5 create_datawriter — erzeugen

**Spec:** §2.2.2.4.1.5, S. 56 — "This operation creates a DataWriter. The
returned DataWriter will be attached and belongs to the Publisher. The
DataWriter returned by the create_datawriter operation will in fact be a
derived class, specific to the data-type associated with the Topic … In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`,
`WriterRegistration`)

**Tests:** `crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/tests/enable_after_create.rs::a_writer_created_disabled_is_announced_by_its_enable_with_the_qos_it_has_then`

**Status:** done — Rückgabe `Result<DataWriter<T>>`; abgewiesen werden u. a.
ein gelöschter Publisher, eine nicht ausgeführte DATA_REPRESENTATION und ein
PERSISTENT-Writer ohne Speicher.

### §2.2.2.4.1.5 create_datawriter — Listener und Maske

**Spec:** §2.2.2.4.1.5, S. 55 — Parameter `a_listener: DataWriterListener`,
`mask: StatusKind []`.

**Repo:** `create_datawriter_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_datawriter_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.4.1.5 create_datawriter — DATAWRITER_QOS_DEFAULT

**Spec:** §2.2.2.4.1.5, S. 56 — "The special value DATAWRITER_QOS_DEFAULT can be
used to indicate that the DataWriter should be created with the default
DataWriter QoS set in the factory."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — `DATAWRITER_QOS_DEFAULT` nimmt
`get_default_datawriter_qos`.

### §2.2.2.4.1.5 create_datawriter — DATAWRITER_QOS_USE_TOPIC_QOS

**Spec:** §2.2.2.4.1.5, S. 57 — "The special value DATAWRITER_QOS_USE_TOPIC_QOS
can be used to indicate that the DataWriter should be created with a combination
of the default DataWriter QoS and the Topic QoS."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`
(`EndpointQosArg::UseTopicQos`)

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — der Default des Publisher, darüber die Topic-QoS mit
`copy_from_topic_qos`.

### §2.2.2.4.1.5 create_datawriter — Topic eines anderen DomainParticipant

**Spec:** §2.2.2.4.1.5, S. 57 — "The Topic passed to this operation must have
been created from the same DomainParticipant that was used to create this
Publisher. If the Topic was created from a different DomainParticipant, the
operation will fail and return a nil result."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`
(`Topic::created_by_another_than`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::an_endpoint_cannot_use_the_topic_of_another_participant`

**Status:** `done` — `PreconditionNotMet`, kein Writer.

### §2.2.2.4.1.6 delete_datawriter — löschen

**Spec:** §2.2.2.4.1.6, S. 57 — "This operation deletes a DataWriter that
belongs to the Publisher."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`,
`WriterInner::delete`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`

**Status:** done

### §2.2.2.4.1.6 delete_datawriter — anderer Publisher

**Spec:** §2.2.2.4.1.6, S. 57 — "If delete_datawriter is called on a different
Publisher, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_writer_of_another_publisher_is_left_as_it_was`

**Status:** done

### §2.2.2.4.1.6 delete_datawriter — Instanzen abmelden und verwerfen

**Spec:** §2.2.2.4.1.6, S. 57 — "The deletion of the DataWriter will
automatically unregister all instances. Depending on the settings of the
WRITER_DATA_LIFECYCLE QosPolicy, the deletion of the DataWriter may also dispose
all instances."

**Repo:** `crates/dcps/src/publisher.rs` (`WriterInner::delete`,
`WriterInner::unregister_all_instances`); Schalter
`crates/dcps/src/runtime.rs` (`RuntimeConfig::writer_unregister_on_delete`,
Default `true`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_disposes_and_unregisters_its_instances`

**Status:** done — mit `writer.unregister_on_delete = false` sendet ZeroDDS
die Abmeldungen nicht (für `Test_FinalInstanceState_2` der OMG-Suite); der
Default folgt der Spec.

### §2.2.2.4.1.7 lookup_datawriter

**Spec:** §2.2.2.4.1.7, S. 57 — "This operation retrieves a previously created
DataWriter belonging to the Publisher that is attached to a Topic with a
matching topic_name. If no such DataWriter exists, the operation will return
’nil.’ If multiple DataWriter attached to the Publisher satisfy this condition,
then the operation will return one of them."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::lookup_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::lookup_datawriter_hands_out_a_handle_to_the_same_writer`

**Status:** done — die Rust-Operation ist zusätzlich über den Typ `T`
gefiltert.

### §2.2.2.4.1.8 suspend_publications

**Spec:** §2.2.2.4.1.8, S. 57 — "It is a hint to the Service so it can optimize
its performance by e.g., holding the dissemination of the modifications and then
batching them. It is not required that the Service use this hint in any way …
If the Publisher is deleted before resume_publications is called, any suspended
updates yet to be published will be discarded."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::suspend_publications`,
`Publisher::is_suspended`)

**Tests:**
`crates/dcps/src/publisher.rs::tests::suspend_publications_sets_flag`,
`crates/dcps/src/publisher.rs::tests::two_suspends_want_two_resumes`

**Status:** `done` — ein Zähler; der Sendepfad wertet ihn nicht aus, es werden
keine Änderungen zurückgehalten.

### §2.2.2.4.1.9 resume_publications

**Spec:** §2.2.2.4.1.9, S. 57 — "This operation indicates to the Service that
the application has completed the multiple changes initiated by the previous
suspend_publications."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::resume_publications`)

**Tests:** `crates/dcps/src/publisher.rs::tests::resume_publications_clears_flag`

**Status:** done

### §2.2.2.4.1.9 resume_publications — PRECONDITION_NOT_MET ohne suspend

**Spec:** §2.2.2.4.1.9, S. 57–58 — "The call to resume_publications must match a
previous call to suspend_publications. Otherwise the operation will return the
error PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::resume_publications`,
`crates/zerodds-c-api/src/publisher_ffi.rs::zerodds_pub_resume_publications`

**Tests:**
`crates/dcps/src/publisher.rs::tests::resume_without_suspend_is_precondition_not_met`

**Status:** `done` — `resume_publications` ohne offenes `suspend_publications`
ist `PreconditionNotMet`, auch im C-API.

### §2.2.2.4.1.10 begin_coherent_changes — kohärenter Satz

**Spec:** §2.2.2.4.1.10, S. 58 — "This operation requests that the application
will begin a ‘coherent set’ of modifications using DataWriter objects attached
to the Publisher … the receiver will only be able to access the data after all
the modifications in the set are available at the receiver end."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::begin_coherent_changes`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::begin_user_writer_coherent_set`);
`crates/dcps/src/group_order.rs`

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/tests/coherent_changes_e2e.rs::two_coherent_sets_are_released_one_at_a_time`,
`crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_coherent_reader_still_waits_for_end_coherent_changes`

**Status:** done — Ende des Writer-Satzes und dispose/unregister im Satz nach
DDSI-RTPS 2.5 Table 9.22 seit `a5cdaa56247` und `c35d2137e80`; der
GROUP-Pfad ist in `internal/rtps/group-access-followup.md` beschrieben.

### §2.2.2.4.1.10 begin_coherent_changes — unvollständiger Satz

**Spec:** §2.2.2.4.1.10, S. 58 — "In the event that such a change prevents an
entity from receiving the entire set of coherent changes, that entity must
behave as if it had received none of the set."

**Repo:** `crates/dcps/src/subscriber.rs` (Verwerfen unvollständiger Sätze);
`crates/dcps/src/group_order.rs`

**Tests:** `crates/dcps/src/subscriber.rs::tests::an_incomplete_set_is_discarded`,
`crates/dcps/src/group_order.rs::tests::a_set_short_of_one_member_is_discarded_and_counted`

**Status:** done — seit `402f4fd66e2`.

### §2.2.2.4.1.10 begin_coherent_changes — verschachtelt

**Spec:** §2.2.2.4.1.10, S. 58 — "These calls can be nested. In that case, the
coherent set terminates only with the last call to end_coherent_ changes."

**Repo:** `crates/dcps/src/publisher.rs` (`PublisherInner::coherent_depth`,
`begin_coherent_changes`, `end_coherent_changes`)

**Tests:**
`crates/dcps/src/publisher.rs::tests::nested_coherent_changes_end_with_the_last_end`

**Status:** `done` — die Aufrufe zählen; erst das letzte `end_coherent_changes`
schließt den Satz.

### §2.2.2.4.1.11 end_coherent_changes

**Spec:** §2.2.2.4.1.11, S. 58 — "This operation terminates the ‘coherent set’
initiated by the matching call to begin_coherent_ changes. If there is no
matching call to begin_coherent_ changes, the operation will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::end_coherent_changes`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::end_coherent_changes_without_begin_is_precondition_not_met`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`

**Status:** done

### §2.2.2.4.1.12 wait_for_acknowledgments (Publisher)

**Spec:** §2.2.2.4.1.12, S. 58 — "This operation blocks the calling thread until
either all data written by the reliable DataWriter entities is acknowledged by
all matched reliable DataReader entities, or else the duration specified by the
max_wait parameter elapses".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::wait_for_acknowledgments`)

**Tests:** `crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — der Publisher wartet auf alle seine Writer; ein Reader, der nicht bestätigt, ergibt TIMEOUT nach der angegebenen Zeit.

### §2.2.2.4.1.13 get_participant (Publisher)

**Spec:** §2.2.2.4.1.13, S. 58 — "This operation returns the DomainParticipant
to which the Publisher belongs."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::get_participant`

**Tests:**
`crates/dcps/tests/default_qos.rs::publisher_and_subscriber_know_their_participant`

**Status:** `done` — `None`, sobald kein Handle des Participant mehr besteht.

### §2.2.2.4.1.14 delete_contained_entities (Publisher)

**Spec:** §2.2.2.4.1.14, S. 58–59 — "This operation deletes all the entities
that were created by means of the “create” operations on the Publisher. That is,
it deletes all contained DataWriter objects."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::publisher_delete_contained_entities_deletes_every_writer`

**Status:** done

### §2.2.2.4.1.14 delete_contained_entities (Publisher) — PRECONDITION_NOT_MET

**Spec:** §2.2.2.4.1.14, S. 58 — "The operation will return
PRECONDITION_NOT_MET if the any of the contained entities is in a state where
it cannot be deleted."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET, solange ein GROUP-Publisher in einem kohärenten Satz ist.

### §2.2.2.4.1.15 set_default_datawriter_qos

**Spec:** §2.2.2.4.1.15, S. 59 — "This operation sets a default value of the
DataWriter QoS policies which will be used for newly created DataWriter entities
in the case where the QoS policies are defaulted in the create_datawriter
operation … if they are not, the operation will have no effect and return
INCONSISTENT_POLICY … DATAWRITER_QOS_DEFAULT may be passed".

**Repo:** `crates/dcps/src/publisher.rs::Publisher::set_default_datawriter_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — prüft die Konsistenz wie `create_datawriter`;
`DATAWRITER_QOS_DEFAULT` setzt zurück.

### §2.2.2.4.1.16 get_default_datawriter_qos

**Spec:** §2.2.2.4.1.16, S. 59 — "The values retrieved by
get_default_datawriter_qos will match the set of values specified on the last
successful call to set_default_datawriter_qos, or else, if the call was never
made, the default values listed in the QoS table in 2.2.3".

**Repo:** `crates/dcps/src/publisher.rs::Publisher::get_default_datawriter_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — wie `get_default_publisher_qos`.

### §2.2.2.4.1.17 copy_from_topic_qos (Publisher)

**Spec:** §2.2.2.4.1.17, S. 59 — "This operation copies the policies in the
a_topic_qos to the corresponding policies in the a_datawriter_qos (replacing
values in the a_datawriter_qos, if present) … This operation does not check the
resulting a_datawriter_qos for consistency."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::copy_from_topic_qos`)

**Tests:** `crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_copies_every_policy_both_carry`,
`crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_does_not_touch_writer_only_policies`,
`crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_copies_durability_and_reliability`

**Status:** done — die zwölf Policies, die beide QoS-Sätze nach §2.2.3
tragen; TOPIC_DATA ist keine DataWriter-Policy und wird nicht kopiert.

### §2.2.2.4.2 DataWriter — ein Publisher, ein Topic, ein Typ

**Spec:** §2.2.2.4.2, S. 61 — "A DataWriter is attached to exactly one Publisher
that acts as a factory for it. A DataWriter is bound to exactly one Topic and
therefore to exactly one data type. The Topic must exist prior to the
DataWriter’s creation. DataWriter is an abstract class. It must be specialized
for each particular application data-type".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`, `DataWriter::get_publisher`,
`DataWriter::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`,
`crates/dcps/src/publisher.rs::tests::datawriter_type_name_follows_the_topic_type`

**Status:** done

### §2.2.2.4.2 DataWriter — NOT_ENABLED

**Spec:** §2.2.2.4.2, S. 62 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable, and
get_statuscondition may return the value NOT_ENABLED."

**Repo:** `crates/dcps/src/publisher.rs` (`EntityState::check_enabled` in
`write`, `register_instance_w_timestamp`, `unregister_instance_w_timestamp`,
`dispose_w_timestamp`, `get_key_value`, `wait_for_acknowledgments`, den
get-status-Operationen)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`,
`crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — zum Widerspruch dieser Regel mit §2.2.2.1.1.7 siehe dort.

### §2.2.2.4.2.1 set_listener (DataWriter)

**Spec:** §2.2.2.4.2.1, S. 62 — "a DataWriter can be attached to a Listener at
creation time or later by using the set_listener operation. The attached
Listener must extend DataWriterListener."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_consumed_at_writer`

**Status:** done — nachträglich; beim Erzeugen nicht (siehe §2.2.2.4.1.5).

### §2.2.2.4.2.2 get_listener (DataWriter)

**Spec:** §2.2.2.4.2.2, S. 62 — "Allows access to the attached
DataWriterListener."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.4.2.3 set_qos (DataWriter)

**Spec:** §2.2.2.4.2.3, S. 62 — "a DataWriter can be given QoS at creation time
or later by using the set_qos operation … Possible error codes returned in
addition to the standard ones: IMMUTABLE_POLICY, INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_qos`,
`runtime_qos_update`); `crates/dcps/src/qos.rs` (`immutable_writer_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done — INCONSISTENT_POLICY fehlt (§2.2.2.1.1.1).

### §2.2.2.4.2.4 get_qos (DataWriter)

**Spec:** §2.2.2.4.2.4, S. 62 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_qos`, `DataWriter::qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`

**Status:** done

### §2.2.2.4.2.5 register_instance — Handle, idempotent

**Spec:** §2.2.2.4.2.5, S. 62–63 — "It takes as a parameter an instance (to get
the key value) and returns a handle that can be used in successive write or
dispose operations … The special value HANDLE_NIL may be returned by the Service
if it does not want to allocate any handle for that instance … The operation
register_instance is idempotent. If it is called for an already registered
instance, it just returns the already allocated handle." Die Operation darf wie
`write` TIMEOUT und OUT_OF_RESOURCES liefern.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_instance`,
`DataWriter::register_instance_w_timestamp`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::register`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_instance_returns_stable_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_finds_registered_handle`

**Status:** done — für einen Typ ohne Schlüssel liefert die Operation
`HANDLE_NIL`; sie blockiert nicht und prüft `max_instances` nicht (die Spec
erlaubt TIMEOUT/OUT_OF_RESOURCES, verlangt sie nicht).

### §2.2.2.4.2.5 register_instance — optional, write mit HANDLE_NIL

**Spec:** §2.2.2.4.2.5, S. 63 — "The explicit use of this operation is optional
as the application may call directly the write operation and specify a
HANDLE_NIL to indicate that the ‘key’ should be examined to identify the
instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_on_write`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::write_w_timestamp_auto_registers_unknown_instance`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`

**Status:** done — `write` hat keinen Handle-Parameter und registriert die
Instanz aus dem Schlüssel (siehe §2.2.2.4.2.11 „Handle-Parameter“).

### §2.2.2.4.2.6 register_instance_w_timestamp

**Spec:** §2.2.2.4.2.6, S. 63 — "This operation performs the same function as
register_instance and can be used instead of register_instance in the cases
where the application desires to specify the value for the source_timestamp."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_instance_w_timestamp`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_with_timestamp_records_timestamp`

**Status:** done — der Zeitstempel wird im Instanz-Zustand vermerkt; eine
Registrierung geht nicht auf die Leitung.

### §2.2.2.4.2.7 unregister_instance — abmelden

**Spec:** §2.2.2.4.2.7, S. 63–64 — "This operation reverses the action of
register_instance … This operation informs the Service that the DataWriter is
not intending to modify any more of that data instance … DataReader entities
that are reading the instance will eventually receive a sample with a
NOT_ALIVE_NO_WRITERS instance state if no other DataWriter entities are writing
the instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::unregister_instance`,
`DataWriter::unregister_instance_w_timestamp`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::write_user_lifecycle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_unregister_drops_to_no_writers_when_count_zero`,
`crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** done — mit `autodispose_unregistered_instances = TRUE` wird die
Instanz zusätzlich verworfen (§2.2.3.21).

### §2.2.2.4.2.7 unregister_instance — HANDLE_NIL und unbekannter Handle

**Spec:** §2.2.2.4.2.7, S. 63 — "The special value HANDLE_NIL can be used for the
parameter handle. This indicates that the identity of the instance should be
automatically deduced from the instance … If the handle does not correspond to
an existing instance the behavior is in general unspecified, but if detectable
by the Service implementation, the operation shall fail and return the
error-code 'BAD_PARAMETER.'"

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::resolve_handle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_unknown_handle_errors`

**Status:** done — `unregister_instance` und `dispose` teilen
`resolve_handle`; die Tests laufen über `dispose`.

### §2.2.2.4.2.7 unregister_instance — Handle einer anderen Instanz

**Spec:** §2.2.2.4.2.7, S. 63 — "If the handle corresponds to an existing
instance but does not correspond to the same instance referred by the
'instance' parameter, the behavior is in general unspecified, but if detectable
by the Service implementation, the operation shall fail and return the
error-code PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs::DataWriter::resolve_handle`

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_mismatch_errors`

**Status:** `done` — der Handle einer anderen bestehenden Instanz ist
`PreconditionNotMet`, ein Handle ohne Instanz `BadParameter`, für
`unregister_instance` und `dispose`.

### §2.2.2.4.2.7 unregister_instance — erneut registrieren

**Spec:** §2.2.2.4.2.7, S. 63 — "If after that, the application wants to modify
(write or dispose) the instance, it has to register it again, or else use the
special handle value HANDLE_NIL."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_on_write`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::register`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.4.2.7 unregister_instance — Eigentum abgeben

**Spec:** §2.2.2.4.2.7, S. 64 — "If the DataWriter was the exclusive owner of
the instance, then calling unregister_instance will relinquish that ownership."

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`: Marker mit Writer,
`release_owner_of` bei unregister); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::release_owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`

**Status:** `done` — ein Marker trägt die GUID seines Writers; meldet der
Eigentümer die Instanz ab, ist sie ohne Eigentümer, und der nächste Writer
übernimmt sie.

### §2.2.2.4.2.7 unregister_instance — TIMEOUT

**Spec:** §2.2.2.4.2.7, S. 64 — "This operation may block and return TIMEOUT
under the same circumstances described for the write operation … Possible error
codes returned in addition to the standard ones: TIMEOUT,
PRECONDITION_NOT_MET."

**Repo:**
`crates/dcps/src/publisher.rs::DataWriter::unregister_instance_w_timestamp`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::an_unregister_a_full_reader_holds_back_times_out`

**Status:** `done` — erst die Marke, dann der Zustand: läuft das Senden in die
Grenze, ist das Ergebnis TIMEOUT und die Instanz bleibt registriert.

### §2.2.2.4.2.8 unregister_instance_w_timestamp

**Spec:** §2.2.2.4.2.8, S. 64 — "This operation performs the same function as
unregister_instance and can be used instead of unregister_instance in the cases
where the application desires to specify the value for the source_timestamp."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_timestamps_of_dispose_and_unregister_reach_the_reader`

**Status:** done — der Test deckt es ab.

### §2.2.2.4.2.9 get_key_value (DataWriter)

**Spec:** §2.2.2.4.2.9, S. 64 — "This operation can be used to retrieve the
instance key that corresponds to an instance_handle. The operation will only
fill the fields that form the key inside the key_holder instance. This operation
may return BAD_PARAMETER if the InstanceHandle_t a_handle does not correspond to
an existing data-object known to the DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_key_value`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_get_key_value_extracts_only_key`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_get_key_value_unknown_handle_errors`,
`crates/dcps/src/subscriber.rs::tests::get_key_value_uses_holder_decoder_for_writer_and_reader`

**Status:** done — Rückgabe `Result<T>` statt eines inout-Parameters.

### §2.2.2.4.2.9 get_key_value (DataWriter) — UNSUPPORTED

**Spec:** §2.2.2.1.1, S. 17 — UNSUPPORTED "Can only be returned by operations
that are optional."; §2.2.2.4.2.9, S. 64 nennt nur BAD_PARAMETER.

**Repo:** —

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::get_key_value_of_an_instance_known_by_its_hash`,
`crates/dcps/tests/keyless_instance.rs::get_key_value_answers_for_the_one_instance_of_a_keyless_topic`,
`crates/zerodds-c-api/tests/get_key_value_from_hash.rs::the_key_comes_out_of_a_hash_that_spells_it_and_nothing_else_does`

**Status:** `done` — kennt man zum Handle nur den KeyHash, wird der Schlüssel daraus gebildet, wo der Hash ihn ausschreibt (DDSI-RTPS 2.5 §9.6.4.8 Schritt 5.1), sonst `BadParameter`; der Writer eines Topics ohne Schlüssel antwortet mit seinem ersten Sample. Im C-API (`zerodds_dw_get_key_value`, `zerodds_dr_get_key_value`) ebenso, mit dem Typ des typisierten Endpunkts; ein Endpunkt ohne Typ liefert `BadParameter`.

### §2.2.2.4.2.10 lookup_instance (DataWriter)

**Spec:** §2.2.2.4.2.10, S. 64 — "This operation takes as a parameter an
instance and returns a handle that can be used in subsequent operations … This
operation does not register the instance in question. If the instance has not
been previously registered, or if for any other reason the Service is unable to
provide an instance handle, the Service will return the special value
HANDLE_NIL."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::lookup_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_finds_registered_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_returns_nil_for_unknown`

**Status:** done

### §2.2.2.4.2.11 write — Wert und Quellzeit

**Spec:** §2.2.2.4.2.11, S. 64 — "This operation modifies the value of a data
instance. When this operation is used, the Service will automatically supply the
value of the source_timestamp that is made available to DataReader objects by
means of the source_timestamp attribute inside the SampleInfo … This operation
must be provided on the specialized class that is generated for the particular
application data-type".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`,
`DataWriter::write_with_source_ts_override`)

**Tests:** `crates/dcps/src/publisher.rs::tests::datawriter_write_queues_encoded_sample`,
`crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`,
`crates/dcps/tests/time_source_destination_order.rs::test_clock_drives_destination_order_deterministically`

**Status:** done — die Quellzeit kommt aus `RuntimeConfig::time_source`.

### §2.2.2.4.2.11 write — Liveliness

**Spec:** §2.2.2.4.2.11, S. 65 — "As a side effect, this operation asserts
liveliness on the DataWriter itself, the Publisher and the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (Schreibpfad: `last_liveliness_assert`,
`DcpsRuntime::note_participant_liveliness`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_writer_goes_not_alive_when_silent_and_back_when_it_writes`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_sees_writer_alive_again_after_resumed_publishing`

**Status:** done — der Publisher hat keinen eigenen Liveliness-Zustand.

### §2.2.2.4.2.11 write — Handle-Parameter

**Spec:** §2.2.2.4.2.11, S. 65 — "The special value HANDLE_NIL can be used for
the parameter handle … If the handle corresponds to an existing instance but
does not correspond to the same instance referred by the ‘data’ parameter … the
operation shall fail and return the error-code PRECONDITION_NOT_MET. If the
handle does not correspond to an existing instance … BAD_PARAMETER."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write_with_handle`, `write_w_timestamp_with_handle`, `check_write_handle`); `crates/zerodds-c-api/src/publisher_ffi.rs` (`zerodds_dw_write`, `zerodds_dw_write_w_timestamp`, `write_handle_refused`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::write_with_a_handle_follows_the_handle_rules`, `crates/zerodds-c-api/tests/write_handle.rs::a_write_handle_must_be_the_one_of_the_key`

**Status:** done — `write` bleibt HANDLE_NIL; `write_with_handle` und `write_w_timestamp_with_handle` nehmen den Handle: der Handle des Schlüssels geht durch, der einer anderen bestehenden Instanz ist `PreconditionNotMet`, einer ohne Instanz `BadParameter`. Die C-ABI las `handle` nicht (`_handle`) und prüft ihn jetzt ebenso.

### §2.2.2.4.2.11 write — Blockieren bis max_blocking_time, TIMEOUT

**Spec:** §2.2.2.4.2.11, S. 65 — "If the RELIABILITY kind is set to RELIABLE, the
write operation may block if the modification would cause data to be lost or
else cause one of the limits specified in the RESOURCE_LIMITS to be exceeded …
If max_blocking_time elapses before the DataWriter is able to store the
modification without exceeding the limits, the write operation will fail and
return TIMEOUT."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_writer_resource_limits`)

**Tests:** `crates/dcps/src/publisher.rs::tests::write_blocks_until_drain_when_reliable_max_samples_reached`,
`crates/dcps/src/runtime.rs::tests::a_limited_writer_with_an_acknowledging_reader_writes_everything`,
`crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** done

### §2.2.2.4.2.11 write — OUT_OF_RESOURCES statt Blockieren

**Spec:** §2.2.2.4.2.11, S. 65 — "Instead of blocking, the write operation is
allowed to return immediately with the error code OUT_OF_RESOURCES provided the
following two conditions are met: 1. The reason for blocking would be that the
RESOURCE_LIMITS are exceeded. 2. The service determines that waiting the
max_waiting_time has no chance of freeing the necessary resources."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_write_no_acknowledgement_can_make_room_for_is_refused_at_once`,
`crates/dcps/src/publisher.rs::tests::write_returns_oor_when_best_effort_queue_full`

**Status:** done

### §2.2.2.4.2.12 write_w_timestamp

**Spec:** §2.2.2.4.2.12, S. 65–66 — "This operation performs the same function as
write except that it also provides the value for the source_timestamp that is
made available to DataReader objects by means of the source_timestamp attribute
inside the SampleInfo." Handle-Regeln, TIMEOUT, OUT_OF_RESOURCES wie bei write.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write_w_timestamp`)

**Tests:** `crates/dcps/tests/intra_runtime_timestamp_pairing.rs::write_w_timestamp_stamps_the_sample_with_the_given_timestamp_not_the_clock`,
`crates/dcps/tests/sample_info_lifecycle.rs::write_w_timestamp_auto_registers_unknown_instance`

**Status:** done — `write_w_timestamp_with_handle` nimmt den Handle wie `write_with_handle` (siehe §2.2.2.4.2.11).

### §2.2.2.4.2.13 dispose — Instanz verwerfen

**Spec:** §2.2.2.4.2.13, S. 66 — "This operation requests the middleware to
delete the data … applications are made aware of the deletion by means of
operations on the DataReader objects that already knew that instance … This
operation does not modify the value of the instance. The instance parameter is
passed just for the purposes of identifying the instance. When this operation is
used, the Service will automatically supply the value of the source_timestamp".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose`,
`DataWriter::dispose_w_timestamp`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::write_user_lifecycle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`,
`crates/dcps/src/runtime.rs::tests::qr_dispose_delivers_lifecycle_to_intra_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`

**Status:** done

### §2.2.2.4.2.13 dispose — Handle-Regeln

**Spec:** §2.2.2.4.2.13, S. 66 — "The constraints on the values of the handle
parameter and the corresponding error behavior are the same specified for the
unregister_instance operation".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::resolve_handle`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_unknown_handle_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_unregistered_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_mismatch_errors`

**Status:** `done` — dieselben Regeln wie `unregister_instance`: HANDLE_NIL, ein
unbekannter Handle `BadParameter`, der Handle einer anderen bestehenden Instanz
`PreconditionNotMet`.

### §2.2.2.4.2.13 dispose — TIMEOUT und OUT_OF_RESOURCES

**Spec:** §2.2.2.4.2.13, S. 66 — "This operation may block and return TIMEOUT
under the same circumstances described for the write operation … This operation
may return OUT_OF_RESOURCES under the same circumstances described for the write
operation".

**Repo:** `crates/dcps/src/publisher.rs::DataWriter::dispose_w_timestamp`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_a_full_reader_holds_back_times_out`

**Status:** `done` — erst die Marke, dann der Zustand; ein Fehler des Sendens
(TIMEOUT, OUT_OF_RESOURCES) geht an den Aufrufer, die Instanz bleibt ALIVE.

### §2.2.2.4.2.13 dispose — Topic ohne Schlüssel

**Spec:** §2.2.1.2.2, S. 21 — "If no key is provided, the data set associated
with the Topic is restricted to a single instance."; §2.2.2.4.2.13, S. 66 —
dispose verwirft die Instanz.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::instance_key`);
`crates/dcps/src/subscriber.rs` (`DataReader::instance_key_hash`);
`crates/zerodds-c-api/src/subscriber_ffi.rs` (`resolve_instances_and_ownership`)

**Tests:** `crates/dcps/tests/keyless_instance.rs` (fünf),
`crates/zerodds-c-api/src/subscriber_ffi.rs::tests::a_keyless_instance_can_be_disposed`,
`crates/zerodds-c-api/src/publisher_ffi.rs::tests::a_keyless_writer_names_its_one_instance_by_the_zero_key`

**Status:** `done` — die eine Instanz eines Topics ohne Schlüssel (KeyHash aus 16 Nullen, nicht gesendet) wird vom Writer registriert, gefunden, verworfen und abgemeldet; der Reader führt sie im `InstanceTracker` mit Handle, NEW beim ersten Sample, NOT_ALIVE_DISPOSED und NOT_ALIVE_NO_WRITERS, in Rust und im C-API. Die Invalid-Data-Sample trägt den zuletzt empfangenen Wert; ein Marker vor jedem Sample wird gemeldet (`reader.lifecycle.unknown_instance`) und nicht aufgenommen. `DataWriter::get_key_value` liefert für ein Topic ohne Schlüssel `Unsupported`.

### §2.2.2.4.2.14 dispose_w_timestamp

**Spec:** §2.2.2.4.2.14, S. 66 — "This operation performs the same functions as
dispose except that the application provides the value for the
source_timestamp that is made available to DataReader objects by means of the
source_timestamp attribute inside the SampleInfo".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_timestamps_of_dispose_and_unregister_reach_the_reader`

**Status:** done — der Test deckt es ab.

### §2.2.2.4.2.15 wait_for_acknowledgments (DataWriter)

**Spec:** §2.2.2.4.2.15, S. 67 — "This operation is intended to be used only if
the DataWriter has RELIABILITY QoS kind set to RELIABLE. Otherwise the operation
will return immediately with RETCODE_OK. The operation wait_for_acknowledgments
blocks the calling thread until either all data written by the DataWriter is
acknowledged by all matched DataReader entities that have RELIABILITY QoS kind
RELIABLE, or else the duration specified by the max_wait parameter elapses …
TIMEOUT indicates that max_wait elapsed".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::wait_for_acknowledgments`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_writer_all_acknowledged`,
`DcpsRuntime::wait_ack_event`)

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`,
`crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — „alles bestätigt → OK“ (`e2e_dcps_api`), TIMEOUT und die sofortige Rückkehr eines BEST_EFFORT-Writers sind getestet.

### §2.2.2.4.2.15 wait_for_acknowledgments — TIMEOUT und BEST_EFFORT

**Spec:** §2.2.2.4.2.15, S. 67 — "Otherwise the operation will return immediately
with RETCODE_OK … a return value of TIMEOUT indicates that max_wait elapsed
before all the data was acknowledged."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::all_samples_acknowledged`,
zählt nur Reader-Proxies mit `is_reliable`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::await_user_writer_acknowledged`); `crates/dcps/src/publisher.rs`
(`DataWriter::wait_for_acknowledgments`, `Err(Timeout)` nach `timeout`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::all_samples_acknowledged_asks_the_reliable_readers_only`,
`crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — der Writer mit einem handgebauten RELIABLE-Reader, der nie bestätigt, liefert TIMEOUT nach mindestens der angegebenen Zeit; der BEST_EFFORT-Writer kehrt sofort mit OK zurück.

### §2.2.2.4.2.16 get_liveliness_lost_status

**Spec:** §2.2.2.4.2.16, S. 67 — "This operation allows access to the
LIVELINESS_LOST communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_liveliness_lost_status`)

**Tests:** `crates/dcps/src/publisher.rs::tests::an_offline_writer_polls_nothing_and_reads_the_default_status`

**Status:** done — Inhalt und Zählung des Status unter §2.2.4.1.

### §2.2.2.4.2.17 get_offered_deadline_missed_status

**Spec:** §2.2.2.4.2.17, S. 67 — "This operation allows access to the
OFFERED_DEADLINE_MISSED communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_offered_deadline_missed_status`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`

**Status:** done

### §2.2.2.4.2.18 get_offered_incompatible_qos_status

**Spec:** §2.2.2.4.2.18, S. 67 — "This operation allows access to the
OFFERED_INCOMPATIBLE_QOS communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_offered_incompatible_qos_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.2.4.2.19 get_publication_matched_status

**Spec:** §2.2.2.4.2.19, S. 67 — "This operation allows access to the
PUBLICATION_MATCHED communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_publication_matched_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done

### §2.2.2.4.2.20 get_topic

**Spec:** §2.2.2.4.2.20, S. 67 — "This operation returns the Topic associated
with the DataWriter. This is the same Topic that was used to create the
DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done — die Operation heißt `topic`.

### §2.2.2.4.2.21 get_publisher

**Spec:** §2.2.2.4.2.21, S. 67 — "This operation returns the Publisher to which
the DataWriter belongs."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_publisher`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done

### §2.2.2.4.2.22 assert_liveliness (DataWriter)

**Spec:** §2.2.2.4.2.22, S. 67 — "This operation manually asserts the liveliness
of the DataWriter … This operation need only be used if the LIVELINESS setting
is either MANUAL_BY_PARTICIPANT or MANUAL_BY_TOPIC. Otherwise, it has no
effect."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::assert_liveliness`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_writer_liveliness_eid`)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`,
`crates/dcps/tests/listener_trigger_c22c.rs::assert_liveliness_offline_no_op`

**Status:** done — ohne Rückgabewert.

### §2.2.2.4.2.23 get_matched_subscription_data

**Spec:** §2.2.2.4.2.23, S. 68 — "This operation retrieves information on a
subscription that is currently “associated” with the DataWriter … The
subscription_handle must correspond to a subscription currently associated with
the DataWriter, otherwise the operation will fail and return BAD_PARAMETER."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::matched_subscription_data`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`,
`crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — die zugehörigen Einzelpunkte sind erledigt; siehe ihre Tests.

### §2.2.2.4.2.23 get_matched_subscription_data — BAD_PARAMETER

**Spec:** §2.2.2.4.2.23, S. 68 — "otherwise the operation will fail and return
BAD_PARAMETER."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — der Test deckt es ab.

### §2.2.2.4.2.24 get_matched_subscriptions

**Spec:** §2.2.2.4.2.24, S. 68 — "This operation retrieves the list of
subscriptions currently “associated” with the DataWriter; that is, subscriptions
that have a matching Topic and compatible QoS that the application has not
indicated should be “ignored” … These handles match the ones that appear in the
‘instance_handle’ field of the SampleInfo when reading the “DCPSSubscriptions”
builtin topic."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::matched_subscriptions`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_writer_matched_subscription_handles`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`, `crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches_over_the_wire`

**Status:** done — die Handles der Liste sind die `instance_handle` im SampleInfo des DCPSSubscription-Readers (in einem Prozess und über das Netz).

### §2.2.2.4.3 PublisherListener

**Spec:** §2.2.2.4.3, S. 68 — "Since a Publisher is a kind of Entity, it has the
ability to have a listener associated with it. In this case, the associated
listener should be of concrete type PublisherListener."

**Repo:** `crates/dcps/src/listener.rs` (`PublisherListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::publisher_listener_is_object_safe`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`

**Status:** done — mit den vier DataWriter-Callbacks (§2.2.4.3).

### §2.2.2.4.4 DataWriterListener

**Spec:** §2.2.2.4.4, S. 69 — Operationen on_liveliness_lost,
on_offered_deadline_missed, on_offered_incompatible_qos,
on_publication_matched, je mit `the_writer: DataWriter` und dem Status.

**Repo:** `crates/dcps/src/listener.rs` (`DataWriterListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datawriter_listener_is_object_safe`,
`crates/dcps/tests/listener_trigger_c22c.rs::offered_deadline_missed_listener_fires_on_delta`,
`crates/dcps/tests/listener_trigger_c22c.rs::liveliness_lost_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::offered_incompatible_qos_dispatcher_fires_listener`,
`crates/dcps/tests/listeners_fire_without_polling.rs::matched_listeners_fire_on_match_and_unmatch_without_a_poll`

**Status:** done — die Callbacks erhalten den Handle des Writers statt des
Writers (§2.2.4.3).

### §2.2.2.4.5 Concurrency Behavior

**Spec:** §2.2.2.4.5, S. 69 — "several DataWriter may operate in different
threads. If they share the same Publisher, the middleware guarantees that its
operations are thread-safe. However, it is not required that each requesting
thread be treated in isolation from the others (leading e.g., to several
isolated sets of coherent changes)."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher`, `DataWriter<T>`: `Send`
und `Sync` über `Arc`/`Mutex`)

**Tests:** `crates/dcps/src/publisher.rs::tests::two_writers_of_one_publisher_write_concurrently`,
`crates/dcps/src/publisher.rs::tests::two_datawriters_on_one_publisher_write_concurrently`

**Status:** done

---

## §2.2.2.5 Subscription Module

### §2.2.2.5 Klassen des Subscription Module

**Spec:** §2.2.2.5, S. 70–71 — Subscriber, DataReader, DataSample, SampleInfo,
SubscriberListener, DataReaderListener, ReadCondition, QueryCondition.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Klassen sind unten einzeln
geführt.

### §2.2.2.5.1 Access to the data — read und take

**Spec:** §2.2.2.5.1, S. 71 — "The general semantics of the “read” operations
is that the application only gets access to the corresponding data; the data
remains the middleware’s responsibility and can be read again. The semantics of
the “take” operation is that the application takes full responsibility for the
data; that data will no longer be accessible to the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`, `DataReader::take`,
`DataReader::read_with_info`, `DataReader::take_with_info`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_read_preserves_samples`,
`crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/query_condition.rs::read_w_condition_preserves_cache_state`

**Status:** done

### §2.2.2.5.1 Access to the data — geordnete Sammlung aus Data und SampleInfo

**Spec:** §2.2.2.5.1, S. 71 — "Each of these operations returns an ordered
collection of Data values and associated SampleInfo objects … Multiple samples
can refer to the same instance if the settings of the HISTORY QoS allow for
it."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_with_info`,
`DataReader::read_with_info`); `crates/dcps/src/sample.rs` (`Sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_n_keeps_the_newest_n_per_instance`

**Status:** done — `read` und `take` ohne Zusatz liefern nur die Daten,
`*_with_info` die Paare.

### §2.2.2.5.1.1 Interpretation of the SampleInfo — Inhalt

**Spec:** §2.2.2.5.1.1, S. 71–72 — die SampleInfo enthält sample_state,
view_state, instance_state, valid_data, disposed_generation_count,
no_writers_generation_count, sample_rank, generation_rank,
absolute_generation_rank, source_timestamp.

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — die Bedeutung der Felder ist unten je Feld geführt.

### §2.2.2.5.1.2 sample_state READ / NOT_READ

**Spec:** §2.2.2.5.1.2, S. 72 — "For each sample received, the middleware
internally maintains a sample_state relative to each DataReader. The
sample_state can either be READ or NOT_READ. READ indicates that the DataReader
has already accessed that sample by means of read. NOT_READ indicates that the
DataReader has not accessed that sample before."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleStateKind`);
`crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/src/sample_info.rs::tests::sample_state_predicate`

**Status:** done

### §2.2.2.5.1.3 instance_state ALIVE

**Spec:** §2.2.2.5.1.3, S. 72 — "ALIVE indicates that (a) samples have been
received for the instance, (b) there are live DataWriter entities writing the
instance, and (c) the instance has not been explicitly disposed (or else more
samples have been received after it was disposed)."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::note_admitted`,
`InstanceTracker::observe_sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`,
`crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`

**Status:** done

### §2.2.2.5.1.3 instance_state NOT_ALIVE_DISPOSED

**Spec:** §2.2.2.5.1.3, S. 72 — "NOT_ALIVE_DISPOSED indicates the instance was
explicitly disposed by a DataWriter by means of the dispose operation."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::dispose`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`

**Status:** done

### §2.2.2.5.1.3 instance_state NOT_ALIVE_NO_WRITERS

**Spec:** §2.2.2.5.1.3, S. 72 — "NOT_ALIVE_NO_WRITERS indicates the instance has
been declared as not-alive by the DataReader because it detected that there are
no live DataWriter entities writing that instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_from`,
`DataReader::apply_unmatched_writers`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::unregister_writer`, `InstanceTracker::drop_writer`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_of_one_of_two_writers_leaves_the_instance_alive`,
`crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`

**Status:** `done` — Gründe: Abmeldung des letzten Writers der Instanz, Wegfall
der Zuordnung, Verlust der Liveliness. Die Abmeldung eines von mehreren Writern
ändert den Zustand nicht.

### §2.2.2.5.1.3 instance_state NOT_ALIVE_NO_WRITERS bei Verlust der Liveliness

**Spec:** §2.2.2.5.1.3, S. 72 — "because it detected that there are no live
DataWriter entities writing that instance"; §2.2.4.2.2, S. 132 — "The loss of
liveliness of the DataWriter of an instance for which there is no other
DataWriter."

**Repo:** `crates/dcps/src/liveliness_tracker.rs`
(`LivelinessTracker::take_lost`); `crates/dcps/src/runtime.rs`
(`check_liveliness`, `DcpsRuntime::take_reader_lost_liveliness_writers`);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`

**Status:** `done` — läuft die Lease eines Writers ab, nimmt der DataReader ihn
aus den Writern seiner Instanzen; eine Instanz ohne Writer erhält die
NOT_ALIVE_NO_WRITERS-Marke. Schreibt der Writer wieder, ist die Instanz ALIVE
(`no_writers_generation_count` 1).

### §2.2.2.5.1.3 instance_state unter OWNERSHIP EXCLUSIVE

**Spec:** §2.2.2.5.1.3, S. 72 — "If OWNERSHIP is set to EXCLUSIVE, then the
instance_state becomes NOT_ALIVE_DISPOSED only if the DataWriter that “owns”
the instance explicitly disposes it. The instance_state becomes ALIVE again only
if the DataWriter that owns the instance writes it."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::passes_exclusive_ownership` für Datensamples, `ingest_pending` für
Marker); `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** `done` — ein `dispose` eines Writers, der die Instanz nicht besitzt,
wird nicht übernommen; ein Datensample macht die Instanz nur vom Eigentümer
ALIVE.

### §2.2.2.5.1.3 instance_state unter OWNERSHIP SHARED

**Spec:** §2.2.2.5.1.3, S. 72 — "If OWNERSHIP is set to SHARED, then the
instance_state becomes NOT_ALIVE_DISPOSED if any DataWriter explicitly disposes
the instance. The instance_state becomes ALIVE as soon as any DataWriter writes
the instance again."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::dispose`,
`InstanceTracker::observe_sample`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`,
`crates/dcps/src/instance_tracker.rs::tests::dispose_transitions_to_disposed`

**Status:** done

### §2.2.2.5.1.3 instance_state als Momentaufnahme beim Lesen

**Spec:** §2.2.2.5.1.3, S. 73 — "The instance_state available in the SampleInfo
is a snapshot of the instance_state of the instance at the time the collection
was obtained (i.e., at the time read or take was called). The instance_state is
therefore be the same for all samples in the returned collection that refer to
the same instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::refresh_states`, gerufen
von `take_filtered`, `read_filtered`, `read_w_condition`, `take_w_condition`,
`read_instance`, `take_instance`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::current_states`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::the_instance_state_of_a_sample_is_that_of_its_instance_when_read`

**Status:** `done` — vor jedem `read`/`take` mit SampleInfo steht in jedem
Sample der Zustand seiner Instanz zu diesem Zeitpunkt; die Zustandsmasken wählen
danach aus. Die ReadCondition wertet die Werte vom letzten `read`/`take` aus
(aus dem Code gelesen).

### §2.2.2.5.1.4 valid_data

**Spec:** §2.2.2.5.1.4, S. 73 — "there are situations where a DataSample
contains only the SampleInfo and does not have any associated data … The
application can distinguish wether a particular DataSample has data by
examining the value of the valid_data flag."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::valid_data`);
`crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`:
`valid_data = false`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_dispose_marker_has_invalid_data`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`

**Status:** done — `read` und `take` ohne Zusatz überspringen solche Samples,
die `*_with_info`-Varianten liefern sie.

### §2.2.2.5.1.5 Generationszähler — Anfangswert

**Spec:** §2.2.2.5.1.5, S. 73 — "The disposed_generation_count and
no_writers_generation_count are initialized to zero when the DataReader first
detects the presence of a never-seen-before instance."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceState` Anfangswerte)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_sample_creates_new_instance_on_first_call`,
`crates/dcps/src/sample_info.rs::tests::defaults_are_alive_new_not_read`

**Status:** done

### §2.2.2.5.1.5 disposed_generation_count

**Spec:** §2.2.2.5.1.5, S. 73 — "The disposed_generation_count is incremented
each time the instance_state of the corresponding instance changes from
NOT_ALIVE_DISPOSED to ALIVE."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::observe_sample`,
`InstanceTracker::register`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`,
`crates/dcps/src/instance_tracker.rs::tests::re_register_after_dispose_bumps_disposed_generation`

**Status:** done

### §2.2.2.5.1.5 no_writers_generation_count

**Spec:** §2.2.2.5.1.5, S. 73 — "The no_writers_generation_count is incremented
each time the instance_state of the corresponding instance changes from
NOT_ALIVE_NO_WRITERS to ALIVE."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — der Test deckt es ab.

### §2.2.2.5.1.5 Generationszähler — Stand beim Empfang

**Spec:** §2.2.2.5.1.5, S. 73 — "The disposed_generation_count and
no_writers_generation_count available in the SampleInfo capture a snapshot of
the corresponding counters at the time the sample was received."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`:
Zähler aus `note_admitted`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.5.1.6 sample_rank

**Spec:** §2.2.2.5.1.6, S. 73 — "The sample_rank indicates the number of samples
of the same instance that follow the current one in the collection."

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_single_instance_three_samples`,
`crates/dcps/src/sample_info.rs::tests::compute_ranks_two_instances_are_independent`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance und die gefilterten Varianten füllen die Ränge mit `compute_ranks`.

### §2.2.2.5.1.6 generation_rank

**Spec:** §2.2.2.5.1.6, S. 73 — "generation_rank = (MRSIC.disposed_generation_count
+ MRSIC.no_writers_generation_count) - (S.disposed_generation_count +
S.no_writers_generation_count)".

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_generation_rank_against_last_sample_in_batch`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance und die gefilterten Varianten füllen die Ränge mit `compute_ranks`.

### §2.2.2.5.1.6 absolute_generation_rank

**Spec:** §2.2.2.5.1.6, S. 74 — "absolute_generation_rank =
(MRS.disposed_generation_count + MRS.no_writers_generation_count) -
(S.disposed_generation_count + S.no_writers_generation_count)".

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_absolute_generation_rank_uses_current_not_batch`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance und die gefilterten Varianten füllen die Ränge mit `compute_ranks`.

### §2.2.2.5.1.7 Interpretation of the SampleInfo counters and ranks

**Spec:** §2.2.2.5.1.7, S. 74 — Erläuterung, wie eine Anwendung Zähler und
Ränge nutzt (sample_rank==0, generation_rank==0, absolute_generation_rank!=0).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anwendungshinweise ohne Anforderung.

### §2.2.2.5.1.8 view_state NEW / NOT_NEW

**Spec:** §2.2.2.5.1.8, S. 74 — "NEW indicates that either this is the first
time that the DataReader has ever accessed samples of that instance, or else
that the DataReader has accessed previous samples of the instance, but the
instance has since been reborn … NOT_NEW indicates that the DataReader has
already accessed samples of the same instance and that the instance has not
been reborn since."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`reader_view_new`,
`InstanceTracker::mark_view_seen`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`,
`crates/dcps/src/instance_tracker.rs::tests::mark_view_seen_clears_new_flag`,
`crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — die zugehörigen Einzelpunkte sind erledigt; siehe ihre Tests.

### §2.2.2.5.1.8 view_state nach Wiedergeburt

**Spec:** §2.2.2.5.1.8, S. 74 — "… or else that the DataReader has accessed
previous samples of the instance, but the instance has since been reborn (i.e.,
become not-alive and then alive again)."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — der Test deckt es ab.

### §2.2.2.5.1.8 view_state als Momentaufnahme beim Lesen

**Spec:** §2.2.2.5.1.8, S. 74 — "The view_state available in the SampleInfo is
a snapshot of the view_state of the instance relative to the DataReader used to
access the samples at the time the collection was obtained … The view_state is
therefore the same for all samples in the returned collection that refer to the
same instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::refresh_states`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::current_states`,
`mark_view_seen`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::the_view_state_of_a_sample_is_that_of_its_instance_when_read`

**Status:** `done` — das erste `read` liefert NEW, ein weiteres `read` derselben
Samples NOT_NEW; alle Samples einer Instanz in einer Sammlung tragen denselben
Wert.

### §2.2.2.5.1.8 Freigabe einer Instanz ohne Writer

**Spec:** §2.2.2.5.1.8, S. 74 — "Once an instance has been detected as not
having any “live” writers and all the samples associated with the instance are
‘taken’ from the DataReader, the middleware can reclaim all local resources
regarding the instance. Future samples will be treated as ‘never seen’."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`);
`crates/dcps/src/instance_tracker.rs` (Autopurge)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::autopurge_no_writers_after_delay`,
`crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`

**Status:** done — die Spec erlaubt die Freigabe; ZeroDDS gibt eine Instanz
nach READER_DATA_LIFECYCLE frei (§2.2.3.22), mit dem Default INFINITE nicht.

### §2.2.2.5.1 Figure 2.11 — Zustandsdiagramm

**Spec:** §2.2.2.5.1, S. 75 — Statechart von instance_state und view_state
einer Instanz.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Übergänge sind in den Items zu
§2.2.2.5.1.3, §2.2.2.5.1.5 und §2.2.2.5.1.8 geführt.

### §2.2.2.5.1.9 Data access patterns — Auswahl nach Zuständen, nicht blockierend

**Spec:** §2.2.2.5.1.9, S. 75 — die Sammlung hängt ab von den gewünschten
sample-, view- und instance-states; "The read and take operations are
non-blocking and just deliver what is currently available that matches the
specified states."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`,
`DataReader::take_filtered`); `crates/dcps/src/sample_info.rs`
(`SampleInfo::matches_states`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`,
`crates/dcps/src/sample_info.rs::tests::matches_states_filter`

**Status:** done

### §2.2.2.5.1.9 Data access patterns — Werte der Zustandsmasken

**Spec:** §2.2.2.5.1.9, S. 75, mit §2.3.3, S. 155 — die Masken der
read/take-Operationen sind `SampleStateMask`, `ViewStateMask`,
`InstanceStateMask` mit `READ_SAMPLE_STATE = 0x0001 << 0`,
`NOT_READ_SAMPLE_STATE = 0x0001 << 1`.

**Repo:** `crates/dcps/src/sample_info.rs` (`sample_state_mask`,
`view_state_mask`, `instance_state_mask`), `crates/dcps/src/psm_constants.rs`
(`state`)

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — `READ = 0x0001`, `NOT_READ = 0x0002`; die Masken der
Lese-Operationen sind die der IDL-PSM.

### §2.2.2.5.1.9 Data access patterns — read_w_condition und take_w_condition

**Spec:** §2.2.2.5.1.9, S. 76 — "The read_w_condition and take_w_condition
operations take a ReadCondition object as a parameter instead of sample, view,
and instance states. The behavior is that the samples returned will only be
those for which the condition is TRUE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`,
`DataReader::take_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/tests/query_condition.rs::take_w_condition_consumes_only_matching_samples`,
`crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`

**Status:** done — die Operationen nehmen eine `ReadCondition` oder eine `QueryCondition`.

### §2.2.2.5.1.9 Muster 1 — GROUP: begin_access, get_datareaders, read/take, end_access

**Spec:** §2.2.2.5.1.9, S. 76 — "General pattern to access samples as a coherent
set and/or in order across DataReader entities. This case applies when
PRESENTATION QoS specifies “access_scope=GROUP.” … if the PRESENTATION QoS
policy specifies ordered_access=TRUE, then the list of DataReaders may return
the same reader several times."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::begin_access`,
`Subscriber::get_datareaders`, `Subscriber::end_access`,
`DataReader::check_group_access`); `crates/dcps/src/group_order.rs`

**Tests:**
`crates/dcps/tests/entity_deletion.rs::under_group_scope_get_datareaders_needs_begin_access`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_lists_a_reader_once_per_sample_in_write_order`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_in_a_group_coherent_set_waits_for_its_end_same_runtime`,
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — das Muster läuft; `get_datareaders` ordnet die Liste unter `ordered_access` nach der GSN (§2.2.2.5.2.10).

### §2.2.2.5.1.9 Muster 2 — ohne GROUP

**Spec:** §2.2.2.5.1.9, S. 76 — "it is not required for the application to call
begin_access and end_access. However, doing so is not an error and it will have
no effect … the return of get_datareaders will be logically a “set” in the sense
that the same reader will not appear twice".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`,
`Subscriber::begin_access`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::get_datareaders_names_the_readers_that_hold_samples_in_the_states`,
`crates/dcps/src/subscriber.rs::tests::subscriber_begin_end_access_roundtrip`

**Status:** done

### §2.2.2.5.1.9 Muster 3 — im SubscriberListener

**Spec:** §2.2.2.5.1.9, S. 77 — Zugriff in `on_data_on_readers`; "The
application can also delegate the accessing of the data to the
DataReaderListener objects installed on each DataReader by calling
notify_datareaders."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::notify_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`,
`crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`

**Status:** done

### §2.2.2.5.2 Subscriber — Empfang der Abonnements

**Spec:** §2.2.2.5.2, S. 77–78 — "A Subscriber is the object responsible for the
actual reception of the data resulting from its subscriptions … it builds the
list of concerned DataReader objects, and then indicates to the application that
data is available, through its listener or by enabling related conditions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Rollenbeschreibung; die Operationen sind
unten geführt, die Regel zu NOT_ENABLED (S. 78) unter §2.2.2.1.1.7.

### §2.2.2.5.2.1 set_listener (Subscriber)

**Spec:** §2.2.2.5.2.1, S. 78 — "a Subscriber can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend SubscriberListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`

**Status:** done — nachträglich; beim Erzeugen nicht (§2.2.2.2.1.3).

### §2.2.2.5.2.2 get_listener (Subscriber)

**Spec:** §2.2.2.5.2.2, S. 78 — "Allows access to the attached
SubscriberListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.5.2.3 set_qos (Subscriber)

**Spec:** §2.2.2.5.2.3, S. 78 — "a Subscriber can be given QoS at creation time
or later by using the set_qos operation … IMMUTABLE_POLICY,
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::subscriber_entity_lifecycle`

**Status:** done

### §2.2.2.5.2.4 get_qos (Subscriber)

**Spec:** §2.2.2.5.2.4, S. 78 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::subscriber_entity_lifecycle`

**Status:** done

### §2.2.2.5.2.5 create_datareader — erzeugen

**Spec:** §2.2.2.5.2.5, S. 78–79 — "This operation creates a DataReader. The
returned DataReader will be attached and belong to the Subscriber. The
DataReader returned by the create_datareader operation will in fact be a
derived class, specific to the data-type associated with the Topic … In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`,
`Subscriber::create_datareader_with_cft`, `Subscriber::create_reader`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_creates_datareader_for_matching_type`,
`crates/dcps/tests/e2e_dcps_api.rs::public_api_create_subscriber_and_reader_offline`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datareader`

**Status:** done — für eine ContentFilteredTopic eine eigene Operation; zu
MultiTopic siehe §2.2.2.3.4.

### §2.2.2.5.2.5 create_datareader — Listener und Maske

**Spec:** §2.2.2.5.2.5, S. 77 — Parameter `a_listener: DataReaderListener`,
`mask: StatusKind []`.

**Repo:** `create_datareader_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_datareader_with_listener` nimmt Listener und Maske; `None` mit einer Maske ist der nil-Listener. Sie erzeugt die Entity und setzt den Listener; was vorher geschah, hört der Listener bei seinem ersten Blick (die Statusmarken bleiben, bis sie gelesen oder von einem Listener genommen werden).

### §2.2.2.5.2.5 create_datareader — DATAREADER_QOS_DEFAULT

**Spec:** §2.2.2.5.2.5, S. 79 — "The special value DATAREADER_QOS_DEFAULT can be
used to indicate that the DataReader should be created with the default
DataReader QoS set in the factory."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::create_datareader`,
`::create_datareader_with_cft`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — `DATAREADER_QOS_DEFAULT` nimmt
`get_default_datareader_qos`.

### §2.2.2.5.2.5 create_datareader — DATAREADER_QOS_USE_TOPIC_QOS

**Spec:** §2.2.2.5.2.5, S. 79 — "Provided that the TopicDescription passed to
this method is a Topic or a ContentFilteredTopic, the special value
DATAREADER_QOS_USE_TOPIC_QOS can be used … It is an error to use
DATAREADER_QOS_USE_TOPIC_QOS when creating a DataReader with a MultiTopic".

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::resolve_reader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — der Default des Subscriber, darüber die QoS des Topic, bei
einem ContentFilteredTopic die des verbundenen Topic. Einen DataReader auf einem
MultiTopic erzeugt die Rust-API nicht (§2.2.2.3.4 ist `open`).

### §2.2.2.5.2.5 create_datareader — TopicDescription eines anderen DomainParticipant

**Spec:** §2.2.2.5.2.5, S. 79 — "The TopicDescription passed to this operation
must have been created from the same DomainParticipant that was used to create
this Subscriber. If the TopicDescription was created from a different
DomainParticipant, the operation will fail and return a nil result."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::create_reader`
(`Topic::created_by_another_than`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::an_endpoint_cannot_use_the_topic_of_another_participant`

**Status:** `done` — `PreconditionNotMet`, kein Reader; bei einem
ContentFilteredTopic zählt das verbundene Topic.

### §2.2.2.5.2.6 delete_datareader — löschen

**Spec:** §2.2.2.5.2.6, S. 79 — "This operation deletes a DataReader that
belongs to the Subscriber … The deletion of a DataReader is not allowed if it
has any outstanding loans as a result of a call to read, take, or one of the
variants thereof."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::subscriber_delete_contained_entities_deletes_readers_and_their_conditions`,
`crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`

**Status:** done — `read` und `take` liefern eigene Werte, es gibt keine
Leihgaben, die das Löschen verhindern könnten.

### §2.2.2.5.2.6 delete_datareader — anderer Subscriber

**Spec:** §2.2.2.5.2.6, S. 79 — "If the DataReader does not belong to the
Subscriber, the operation returns the error PRECONDITION_NOT_MET … If
delete_datareader is called on a different Subscriber, the operation will have
no effect and it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`

**Status:** done

### §2.2.2.5.2.6 delete_datareader — PRECONDITION_NOT_MET mit Conditions

**Spec:** §2.2.2.5.2.6, S. 79 — "The deletion of a DataReader is not allowed if
there are any existing ReadCondition or QueryCondition objects that are attached
to the DataReader … it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`,
`ReaderInner::has_conditions`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`

**Status:** done

### §2.2.2.5.2.7 lookup_datareader

**Spec:** §2.2.2.5.2.7, S. 80 — "This operation retrieves a previously-created
DataReader belonging to the Subscriber that is attached to a Topic with a
matching topic_name. If no such DataReader exists, the operation will return
’nil.’ … The use of this operation on the built-in Subscriber allows access to
the built-in DataReader entities for the built-in topics."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::lookup_datareader`);
`crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::lookup_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::lookup_datareader_hands_out_a_handle_to_the_same_reader`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — `None` für einen unbekannten Namen, auch am Built-in-Subscriber.

### §2.2.2.5.2.8 begin_access — nur unter GROUP nötig, verschachtelbar

**Spec:** §2.2.2.5.2.8, S. 80 — "The application is required to use this
operation only if PRESENTATION QosPolicy of the Subscriber … has the
access_scope set to ‘GROUP.’ … Calling begin_access/end_access in this case is
not considered an error and has no effect. The calls to begin_access/end_access
may be nested."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::begin_access`);
`crates/dcps/src/coherent_set.rs` (`GroupAccessScope`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_begin_access_is_nestable`,
`crates/dcps/src/coherent_set.rs::tests::group_access_nesting`

**Status:** done

### §2.2.2.5.2.8 begin_access — vor den Lese-Operationen unter GROUP

**Spec:** §2.2.2.5.2.8, S. 80 — "the operation begin_access must be called prior
to calling any of the sample-accessing operations, namely: get_datareaders on
the Subscriber and read, take, read_w_condition, take_w_condition on any
DataReader. Otherwise the sample-accessing operations will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`,
`DataReader::check_group_access`)

**Tests:**
`crates/dcps/tests/entity_deletion.rs::under_group_scope_get_datareaders_needs_begin_access`,
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`

**Status:** `done` — `get_datareaders`, `read`, `take`, `read_w_condition` und
`take_w_condition` sind unter GROUP ohne `begin_access` PRECONDITION_NOT_MET; im
Daten-Listener ist kein `begin_access` nötig.

### §2.2.2.5.2.9 end_access

**Spec:** §2.2.2.5.2.9, S. 80 — "This call must close a previous call to
begin_access otherwise the operation will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::end_access`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_end_access_without_begin_returns_precondition_not_met`,
`crates/dcps/src/subscriber.rs::tests::subscriber_too_many_ends_after_balanced_returns_error`

**Status:** done

### §2.2.2.5.2.10 get_datareaders — Reader mit Samples in den Zuständen

**Spec:** §2.2.2.5.2.10, S. 80–81 — "This operation allows the application to
access the DataReader objects that contain samples with the specified
sample_states, view_states, and instance_states … If PRESENTATION access_scope
is INSTANCE or TOPIC the returned collection is a ‘set.’"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::get_datareaders_names_the_readers_that_hold_samples_in_the_states`

**Status:** done — die Masken folgen `sample_info` (siehe §2.2.2.5.1.9 „Werte
der Zustandsmasken“).

### §2.2.2.5.2.10 get_datareaders — GROUP mit ordered_access als Liste

**Spec:** §2.2.2.5.2.10, S. 81 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to TRUE, then the returned collection is a ‘list.’ … the
application should process each DataReader in the same order it appears in the
‘list’ and read or take exactly one sample from each DataReader."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_deletion.rs::group_ordered_access_lists_a_reader_once_per_sample_in_write_order`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** `done` — unter GROUP mit `ordered_access` je Sample ein Eintrag; Samples einer Gruppe nach GSN (`SampleWire::group`), sonst nach Quellzeit.

### §2.2.2.5.2.11 notify_datareaders

**Spec:** §2.2.2.5.2.11, S. 81 — "This operation invokes the operation
on_data_available on the DataReaderListener objects attached to contained
DataReader entities with a DATA_AVAILABLE status that is considered changed".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::notify_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`

**Status:** done

### §2.2.2.5.2.12 get_sample_lost_status (Subscriber)

**Spec:** §2.2.2.5.2.12, S. 81 — "This operation allows access to the
SAMPLE_LOST communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_sample_lost_status`)

**Tests:** `crates/dcps/tests/subscriber_sample_lost.rs::the_subscriber_sums_the_sample_lost_status_of_its_readers`

**Status:** done — der SAMPLE_LOST-Status des Subscriber ist die Summe über seine Reader; das Lesen setzt die Änderung jedes Readers zurück. Die Klassentabelle des Subscriber führt die Operation nicht, der Abschnitt §2.2.2.5.2.12 beschreibt sie.

### §2.2.2.5.2.13 get_participant (Subscriber)

**Spec:** §2.2.2.5.2.13, S. 81 — "This operation returns the DomainParticipant
to which the Subscriber belongs."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::get_participant`

**Tests:**
`crates/dcps/tests/default_qos.rs::publisher_and_subscriber_know_their_participant`

**Status:** `done` — wie am Publisher.

### §2.2.2.5.2.14 delete_contained_entities (Subscriber)

**Spec:** §2.2.2.5.2.14, S. 81 — "it deletes all contained DataReader objects.
This pattern is applied recursively … that is also the QueryCondition and
ReadCondition objects belonging to the contained DataReaders."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::subscriber_delete_contained_entities_deletes_readers_and_their_conditions`

**Status:** done

### §2.2.2.5.2.14 delete_contained_entities (Subscriber) — PRECONDITION_NOT_MET

**Spec:** §2.2.2.5.2.14, S. 81 — "The operation will return
PRECONDITION_NOT_MET if any of the contained entities is in a state where it
cannot be deleted. This will occur, for example, if a contained DataReader
cannot be deleted because the application has called a read or take operation
and has not called the corresponding return_loan operation".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET, solange ein Gruppenzugriff (`begin_access`) offen ist.

### §2.2.2.5.2.15 set_default_datareader_qos

**Spec:** §2.2.2.5.2.15, S. 81–82 — "This operation sets a default value of the
DataReader QoS policies … This operation will check that the resulting policies
are self consistent … DATAREADER_QOS_DEFAULT may be passed".

**Repo:**
`crates/dcps/src/subscriber.rs::Subscriber::set_default_datareader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — wie `set_default_datawriter_qos`; die Konsistenz umfasst
DEADLINE gegen TIME_BASED_FILTER.

### §2.2.2.5.2.16 get_default_datareader_qos

**Spec:** §2.2.2.5.2.16, S. 82 — "This operation retrieves the default value of
the DataReader QoS".

**Repo:**
`crates/dcps/src/subscriber.rs::Subscriber::get_default_datareader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — wie `get_default_publisher_qos`.

### §2.2.2.5.2.17 copy_from_topic_qos (Subscriber)

**Spec:** §2.2.2.5.2.17, S. 82 — "This operation copies the policies in the
a_topic_qos to the corresponding policies in the a_datareader_qos … This
operation does not check the resulting a_datareader_qos for consistency."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::copy_from_topic_qos`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::copy_from_topic_qos_copies_every_policy_both_carry`

**Status:** done — die neun Policies, die beide QoS-Sätze nach §2.2.3 tragen.

### §2.2.2.5.3 DataReader — Abonnement und Datenzugriff

**Spec:** §2.2.2.5.3, S. 82, 86 — "A DataReader allows the application (1) to
declare the data it wishes to receive (i.e., make a subscription) and (2) to
access the data received by the attached Subscriber … DataReader is an abstract
class. It must be specialized for each particular application data-type".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader<T>`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_creates_datareader_for_matching_type`,
`crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`

**Status:** done

### §2.2.2.5.3 DataReader — NOT_ENABLED

**Spec:** §2.2.2.5.3, S. 88 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable, and
get_statuscondition may return the error NOT_ENABLED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_enabled`); in
`crates/dcps/src/subscriber.rs` u. a. `DataReader::take`, `DataReader::read`,
die get-status-Operationen

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`

**Status:** done

### §2.2.2.5.3 DataReader — PRECONDITION_NOT_MET der Lese-Operationen unter GROUP

**Spec:** §2.2.2.5.3, S. 88 — "All sample-accessing operations, namely all
variants of read, take may return the error PRECONDITION_NOT_MET. The
circumstances that result on this are described in 2.2.2.5.2.8."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::check_group_access`,
aufgerufen aus `ingest_into_cache`)

**Tests:**
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`

**Status:** `done` — jede Lese-Operation prüft vor dem Lesen, ob ein
`begin_access` offen ist.

### §2.2.2.5.3.1 set_listener (DataReader)

**Spec:** §2.2.2.5.3.1, S. 88 — "a DataReader can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend DataReaderListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`,
`crates/dcps/src/subscriber.rs::tests::datareader_set_listener_stores_arc_and_mask`

**Status:** done — nachträglich; beim Erzeugen nicht (§2.2.2.5.2.5).

### §2.2.2.5.3.2 get_listener (DataReader)

**Spec:** §2.2.2.5.3.2, S. 88 — "Allows access to the attached
DataReaderListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.5.3.3 set_qos (DataReader)

**Spec:** §2.2.2.5.3.3, S. 88 — "a DataReader can be given QoS at creation time
or later by using the set_qos operation … IMMUTABLE_POLICY,
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::set_qos`);
`crates/dcps/src/qos.rs` (`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done — INCONSISTENT_POLICY fehlt (§2.2.2.1.1.1).

### §2.2.2.5.3.4 get_qos (DataReader)

**Spec:** §2.2.2.5.3.4, S. 88 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_qos`, `DataReader::qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`

**Status:** done

### §2.2.2.5.3.5 create_readcondition

**Spec:** §2.2.2.5.3.5, S. 88 — "This operation creates a ReadCondition. The
returned ReadCondition will be attached and belong to the DataReader. In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`,
`DataReader::holds_matching`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::a_waitset_returns_a_read_condition_of_a_reader_with_data`

**Status:** done

### §2.2.2.5.3.6 create_querycondition

**Spec:** §2.2.2.5.3.6, S. 88–89 — "This operation creates a QueryCondition. The
returned QueryCondition will be attached and belong to the DataReader. The
syntax of the query_expression and query_parameters parameters is described in
Annex B. In case of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_querycondition`);
`crates/dcps/src/condition.rs` (`QueryCondition::new`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`,
`crates/dcps/src/condition.rs::tests::query_condition_rejects_invalid_sql`

**Status:** done

### §2.2.2.5.3.7 delete_readcondition

**Spec:** §2.2.2.5.3.7, S. 89 — "This operation deletes a ReadCondition attached
to the DataReader. Since QueryCondition specializes ReadCondition it can also be
used to delete a QueryCondition. If the ReadCondition is not attached to the
DataReader, the operation will return the error PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::delete_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`

**Status:** done — eine QueryCondition wird über `QueryCondition::base`
gelöscht.

### §2.2.2.5.3.8 read — max_samples

**Spec:** §2.2.2.5.3.8, S. 89 — "This operation accesses a collection of Data
values from the DataReader. The size of the returned collection will be limited
to the specified max_samples."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::max_samples_limits_read_and_take`, `crates/dcps/tests/read_selection.rs::no_matching_sample_is_no_data`,
`crates/dcps/tests/query_condition.rs::max_samples_limits_what_a_condition_read_or_take_returns`

**Status:** done — die Operationen in der Form der Spezifikation (`read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`, `read_w_condition_selected`, `take_w_condition_selected`) nehmen `max_samples` (`LENGTH_UNLIMITED` = -1; darunter `BadParameter`); was nicht geliefert wird, behält seinen Zustand, und bei einem `ORDER BY` gilt die Grenze nach der Ordnung. Die älteren Bequemformen (`read`, `take`, `read_with_info`, ...) liefern alles.

### §2.2.2.5.3.8 read — PRESENTATION INSTANCE: Samples einer Instanz zusammen

**Spec:** §2.2.2.5.3.8, S. 89 — "If PRESENTATION access_scope is INSTANCE, then
the returned collection is a ‘list’ where samples belonging to the same
data-instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::the_default_presentation_groups_the_plain_take_by_instance`,
`crates/dcps/tests/ordered_access_e2e.rs::instance_scope_ordered_access_groups_the_plain_take_by_instance`

**Status:** `done` — die Samples einer Instanz folgen aufeinander, auch mit dem
Default.

### §2.2.2.5.3.8 read — TOPIC ohne ordered_access: Samples einer Instanz zusammen

**Spec:** §2.2.2.5.3.8, S. 89 — "If PRESENTATION access_scope is TOPIC and
ordered_access is set to FALSE, then the returned collection is a ‘list’ where
samples belonging to the same data-instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_without_ordered_access_groups_the_plain_take`

**Status:** `done` — wie bei INSTANCE.

### §2.2.2.5.3.8 read — TOPIC mit ordered_access: Schreibreihenfolge über Instanzen

**Spec:** §2.2.2.5.3.8, S. 89 — "If PRESENTATION access_scope is TOPIC and
ordered_access is set to TRUE, then the returned collection is a ‘list’ were
samples belonging to the same instance may or may not be consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_ordered_access_keeps_write_order_on_the_plain_take`

**Status:** `done`

### §2.2.2.5.3.8 read — GROUP ohne ordered_access: Samples einer Instanz zusammen

**Spec:** §2.2.2.5.3.8, S. 89 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to FALSE, then the returned collection is a ‘list’ where
samples belonging to the same data instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_without_ordered_access_groups_the_plain_take`

**Status:** `done` — wie bei INSTANCE.

### §2.2.2.5.3.8 read — GROUP mit ordered_access: höchstens ein Sample

**Spec:** §2.2.2.5.3.8, S. 89 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to TRUE, then the returned collection contains at most one
sample."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::presentation_limit`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`

**Status:** `done` — `read`, `take`, die `_w_condition`- und
`_instance`-Varianten geben höchstens ein Sample zurück, in der
Schreibreihenfolge über die Instanzen.

### §2.2.2.5.3.8 read — Reihenfolge nach DESTINATION_ORDER

**Spec:** §2.2.2.5.3.8, S. 89 — "If DESTINATION_ORDER is BY_RECEPTION_TIMESTAMP,
samples belonging to the same instances will appear in the relative order in
which there were received … If DESTINATION_ORDER is BY_SOURCE_TIMESTAMP,
samples belonging to the same instances will appear in the relative order
implied by the source_timestamp".

**Repo:** `crates/dcps/src/subscriber.rs` (Aufnahme nach DESTINATION_ORDER in
`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/time_source_destination_order.rs::plain_take_enforces_destination_order`,
`crates/dcps/tests/time_source_destination_order.rs::plain_read_enforces_destination_order`,
`crates/dcps/tests/destination_order_qos.rs::by_source_timestamp_drops_older_samples`

**Status:** done — die QoS selbst ist unter §2.2.3.17 geführt.

### §2.2.2.5.3.8 read — Sammlungen data_values und sample_infos (len, max_len, owns, Leihgabe)

**Spec:** §2.2.2.5.3.8, S. 89–90 — Regeln 1–5 für die Eingabe-Sammlungen
(gleiche Eigenschaften, Leihgabe bei `max_len == 0`, Kopie bei `owns == TRUE`,
`max_samples > max_len` → PRECONDITION_NOT_MET); "PSM mappings that do not
provide these facilities may need to change the signature of the read operation
slightly to compensate for it."

**Repo:** `crates/dcps/src/subscriber.rs` (Rückgabe `Result<Vec<Sample<T>>>`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done — die Rust-API gibt eine eigene `Vec` zurück; es gibt keine
Eingabe-Sammlung und keine Leihgabe, die Regeln 1–5 entfallen nach dem
zitierten Satz.

### §2.2.2.5.3.8 read — Data und SampleInfo eins zu eins

**Spec:** §2.2.2.5.3.8, S. 90 — "On output, the collection of Data values and
the collection of SampleInfo structures are of the same length and are in a
one-to-one correspondence."

**Repo:** `crates/dcps/src/sample.rs` (`Sample { data, info }`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done

### §2.2.2.5.3.8 read — letztes Sample einer nicht lebenden Instanz ohne Daten

**Spec:** §2.2.2.5.3.8, S. 90–91 — "If the instance_state in the SampleInfo is
NOT_ALIVE_DISPOSED or NOT_ALIVE_NO_WRITERS, then the last sample for that
instance in the collection, that is, the one whose SampleInfo has sample_rank==0
does not contain valid data."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_after_the_last_sample_survives_keep_last_1`

**Status:** done

### §2.2.2.5.3.8 read — Samples ohne Daten zählen nicht für RESOURCE_LIMITS

**Spec:** §2.2.2.5.3.8, S. 91 — "Samples that contain no data do not count
towards the limits imposed by the RESOURCE_LIMITS QoS policy."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_marker_does_not_count_against_the_resource_limits`

**Status:** done — der Test deckt es ab.

### §2.2.2.5.3.8 read — sample_state READ, view_state NOT_NEW, instance_state unverändert

**Spec:** §2.2.2.5.3.8, S. 91 — "The act of reading a sample sets its
sample_state to READ. If the sample belongs to the most recent generation of
the instance, it will also set the view_state of the instance to NOT_NEW. It
will not affect the instance_state of the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`,
`DataReader::read`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::mark_view_seen`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** done

### §2.2.2.5.3.8 read — NO_DATA

**Spec:** §2.2.2.5.3.8–§2.2.2.5.3.19, S. 91–95 — für read, take,
read_w_condition, take_w_condition, read_next_sample, take_next_sample,
read_instance, take_instance, read_next_instance, take_next_instance und die
`*_w_condition`-Varianten: "If the DataReader has no samples that meet the
constraints, the return value will be NO_DATA."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::no_matching_sample_is_no_data`, `crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`,
`crates/dcps/tests/query_condition.rs::max_samples_limits_what_a_condition_read_or_take_returns`

**Status:** done — dieselben Operationen liefern `NoData`, wenn kein Sample die Bedingungen erfüllt; die älteren Bequemformen liefern `Ok` mit leerer Liste.

### §2.2.2.5.3.9 take

**Spec:** §2.2.2.5.3.9, S. 91 — "The act of taking a sample removes it from the
DataReader so it cannot be ‘read’ or ‘taken’ again. If the sample belongs to the
most recent generation of the instance, it will also set the view_state of the
instance to NOT_NEW. It will not affect the instance_state of the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take`,
`DataReader::take_with_info`, `DataReader::take_filtered`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`

**Status:** done — Begrenzung und PRESENTATION wie bei `read` (siehe dort).

### §2.2.2.5.3.10 read_w_condition — mit QueryCondition

**Spec:** §2.2.2.5.3.10, S. 91 — "This operation accesses via ‘read’ the samples
that match the criteria specified in the ReadCondition. This operation is
especially useful in combination with QueryCondition to filter data samples
based on the content."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/tests/query_condition.rs::read_w_condition_preserves_cache_state`,
`crates/dcps/tests/query_condition.rs::read_w_condition_unknown_field_drops_sample`

**Status:** done

### §2.2.2.5.3.10 read_w_condition — mit einfacher ReadCondition

**Spec:** §2.2.2.5.3.10, S. 91 — "In case the ReadCondition is a ‘plain’
ReadCondition and not the specialized QueryCondition, the operation is
equivalent to calling read and passing as sample_states, view_states and
instance_states the value of the corresponding attributes in the
read_condition."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`, `ReadConditionArg` in `crates/dcps/src/condition.rs`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`

**Status:** done — eine `ReadCondition` aus `create_readcondition` wählt nach ihren drei Masken; die Samples werden READ (`take_w_condition` entfernt sie).

### §2.2.2.5.3.10 read_w_condition — Condition des Readers, sonst PRECONDITION_NOT_MET

**Spec:** §2.2.2.5.3.10, S. 91 — "The specified ReadCondition must be attached
to the DataReader; otherwise the operation will fail and return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/read_api_spec.rs::a_condition_of_another_reader_is_precondition_not_met`

**Status:** done — eine Condition eines anderen Readers oder ohne Reader liefert PRECONDITION_NOT_MET, für ReadCondition und QueryCondition, `read_w_condition`, `take_w_condition` und `read_next_instance_w_condition`; es wird nichts gelesen.

### §2.2.2.5.3.11 take_w_condition

**Spec:** §2.2.2.5.3.11, S. 91–92 — "This operation is analogous to
read_w_condition except it accesses samples via the ‘take’ operation … The
specified ReadCondition must be attached to the DataReader; otherwise the
operation will fail and return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::take_w_condition_consumes_only_matching_samples`,
`crates/dcps/tests/query_condition.rs::take_w_condition_empty_when_no_match`,
`crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`, `crates/dcps/tests/read_api_spec.rs::a_condition_of_another_reader_is_precondition_not_met`

**Status:** done — mit QueryCondition und einfacher ReadCondition; eine fremde Condition liefert PRECONDITION_NOT_MET.

### §2.2.2.5.3.12 read_next_sample

**Spec:** §2.2.2.5.3.12, S. 92 — "This operation copies the next,
non-previously accessed Data value from the DataReader; the operation also
copies the corresponding SampleInfo … semantically equivalent to the read
operation where the input Data sequence has max_len=1, the
sample_states=NOT_READ, the view_states=ANY_VIEW_STATE, and the
instance_states=ANY_INSTANCE_STATE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_sample`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::read_next_sample_and_take_next_sample_give_one_unaccessed_sample`

**Status:** done — ein Sample, das noch nicht gelesen wurde (NOT_READ), in der Reihenfolge des Eingangs; NO_DATA, wenn keines da ist.

### §2.2.2.5.3.13 take_next_sample

**Spec:** §2.2.2.5.3.13, S. 92 — "This operation copies the next,
non-previously accessed Data value from the DataReader and ‘removes’ it from
the DataReader so it is no longer accessible."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_sample`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::read_next_sample_and_take_next_sample_give_one_unaccessed_sample`

**Status:** done — wie `read_next_sample`, und das Sample verlässt den Reader; bereits gelesene Samples bleiben.

### §2.2.2.5.3.14 read_instance

**Spec:** §2.2.2.5.3.14, S. 92–93 — "The behavior is identical to read except
that all samples returned belong to the single specified instance whose handle
is a_handle … This operation may return BAD_PARAMETER if the InstanceHandle_t
a_handle does not correspond to an existing data-object known to the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_instance`, `read_instance_selected`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_filters_to_one_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_with_handle_nil_errors`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — `read_instance_selected` nimmt `max_samples` und die drei Zustandsmasken; `read_instance` liest alle Samples der Instanz. `HANDLE_NIL` liefert `BadParameter`, ein unbekannter Handle eine leere Liste bzw. NO_DATA.

### §2.2.2.5.3.14 read_instance und take_instance — Zustandsmasken

**Spec:** §2.2.2.5.3.14–§2.2.2.5.3.15, S. 87 und 92–93 — Parameter
`sample_states`, `view_states`, `instance_states`, `max_samples`.

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`, `crates/dcps/tests/read_selection.rs::next_instance_looks_at_the_states`

**Status:** done — `read_instance_selected` und `take_instance_selected` nehmen die drei Masken und `max_samples`; ebenso die `*_next_instance_selected`, deren nächste Instanz das kleinste Handle über `previous` mit Samples in diesen Zuständen ist.

### §2.2.2.5.3.15 take_instance

**Spec:** §2.2.2.5.3.15, S. 93 — "The behavior is identical to take except for
that all samples returned belong to the single specified instance whose handle
is a_handle."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_instance`, `take_instance_selected`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_instance_with_handle_nil_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — wie `read_instance`: `take_instance_selected` mit `max_samples` und den Masken.

### §2.2.2.5.3.16 read_next_instance — nächste Instanz mit Samples

**Spec:** §2.2.2.5.3.16, S. 93 — "The behavior of read_next_instance is ‘as if’
the DataReader invoked read_instance passing the smallest instance_handle among
all the ones that (a) are greater than previous_handle and (b) have available
samples … previous_handle==HANDLE_NIL will return the samples for the instance
which has the smallest instance_handle among all the instances that contain
available samples."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_instance`,
`next_instance_with_samples`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_next_instance_iterates_all`,
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`

**Status:** `done` — die Instanz mit dem kleinsten Handle über
`previous_handle`, die Samples im Cache hat; HANDLE_NIL beginnt bei der
kleinsten.

### §2.2.2.5.3.16 read_next_instance — Instanzen ohne Samples, unbekannter Handle, ordered_access

**Spec:** §2.2.2.5.3.16, S. 93–94 — "(b) have available samples (i.e., samples
that meet the constraints imposed by the specified states) … it is possible to
call the ‘read_next_instance’ operation with a previous_handle that does not
correspond to an instance currently managed by the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::next_instance_with_samples`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`,
`crates/dcps/tests/ordered_access_e2e.rs::take_next_instance_returns_one_instance_under_ordered_access`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — eine Instanz ohne Samples wird übergangen, ein nicht verwalteter `previous_handle` ordnet sich nach seinem Wert ein, unter `ordered_access` kommt eine Instanz; `read_next_instance_selected` nimmt die Masken und `max_samples`.

### §2.2.2.5.3.17 take_next_instance

**Spec:** §2.2.2.5.3.17, S. 94 — "This operation has the same behavior as
read_next_instance except that the samples are ‘taken’ from the DataReader".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_instance`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`

**Status:** `done` — wie `read_next_instance`, mit `take`.

### §2.2.2.5.3.18 read_next_instance_w_condition

**Spec:** §2.2.2.5.3.18, S. 94–95 — "The behavior is identical to
read_next_instance except that all samples returned satisfy the specified
condition."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_instance_w_condition`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::next_instance_w_condition_walks_the_instances_the_condition_accepts`

**Status:** done — die Samples der Instanz mit dem kleinsten Handle über `previous_handle`, die Samples hat, die die Condition annimmt; NO_DATA am Ende. Der Text der Spec sagt hier „>=“ und bei `read_next_instance` „greater“; die Iteration bis NO_DATA braucht „greater“, dem folgt die Implementierung.

### §2.2.2.5.3.19 take_next_instance_w_condition

**Spec:** §2.2.2.5.3.19, S. 95 — "This operation has the same behavior as
read_next_instance_w_condition except that the samples are ‘taken’ from the
DataReader".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_instance_w_condition`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::next_instance_w_condition_walks_the_instances_the_condition_accepts`

**Status:** done — wie `read_next_instance_w_condition`; die angenommenen Samples verlassen den Reader, die übrigen Samples der Instanz bleiben.

### §2.2.2.5.3.20 return_loan

**Spec:** §2.2.2.5.3.20, S. 95 — "This operation indicates to the DataReader
that the application is done accessing the collection of data_values and
sample_infos obtained by some earlier invocation of read or take … calling
return_loan on a collection that does not have a loan is safe and has no side
effects."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::return_loan`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::return_loan_leaves_the_reader_as_it_is`

**Status:** done — `read` und `take` dieser API liefern die Samples als Besitz (`Vec<Sample<T>>`) und leihen nichts vom Reader; `return_loan` hat keine Leihe zurückzugeben und lässt den Reader, wie er ist („calling return_loan on a collection that does not have a loan is safe and has no side effects“). Nach dem Löschen des Readers liefert es `AlreadyDeleted`. Das C-API hat `zerodds_dr_return_loan`.

### §2.2.2.5.3.21 get_liveliness_changed_status

**Spec:** §2.2.2.5.3.21, S. 96 — "This operation allows access to the
LIVELINESS_CHANGED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_liveliness_changed_status`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.2.5.3.22 get_requested_deadline_missed_status

**Spec:** §2.2.2.5.3.22, S. 96 — "This operation allows access to the
REQUESTED_DEADLINE_MISSED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_requested_deadline_missed_status`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done

### §2.2.2.5.3.23 get_requested_incompatible_qos_status

**Spec:** §2.2.2.5.3.23, S. 96 — "This operation allows access to the
REQUESTED_INCOMPATIBLE_QOS communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_requested_incompatible_qos_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.2.5.3.24 get_sample_lost_status

**Spec:** §2.2.2.5.3.24, S. 96 — "This operation allows access to the
SAMPLE_LOST communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_sample_lost_status`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::an_offline_reader_polls_nothing_and_reads_the_default_status`

**Status:** done — Inhalt und Zählung unter §2.2.4.1.

### §2.2.2.5.3.25 get_sample_rejected_status

**Spec:** §2.2.2.5.3.25, S. 96 — "This operation allows access to the
SAMPLE_REJECTED communication status."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::get_sample_rejected_status_reports_the_change_once`

**Status:** done — der Test deckt es ab.

### §2.2.2.5.3.26 get_subscription_matched_status

**Spec:** §2.2.2.5.3.26, S. 96 — "This operation allows access to the
SUBSCRIPTION_MATCHED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_subscription_matched_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.2.5.3.27 get_topicdescription

**Spec:** §2.2.2.5.3.27, S. 96 — "This operation returns the TopicDescription
associated with the DataReader. This is the same TopicDescription that was used
to create the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`,
`crates/dcps/tests/read_api_spec.rs::get_topicdescription_is_the_description_the_reader_was_created_with`

**Status:** done — für einen Reader auf einem Topic das Topic, für einen Reader aus `create_datareader_with_cft` die ContentFilteredTopic.

### §2.2.2.5.3.27 get_topicdescription — ContentFilteredTopic

**Spec:** §2.2.2.5.3.27, S. 96 — "This is the same TopicDescription that was
used to create the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_topicdescription`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::get_topicdescription_is_the_description_the_reader_was_created_with`

**Status:** done — `DataReader::get_topicdescription` liefert die ContentFilteredTopic oder das MultiTopic des Readers; `DataReader::topic` bleibt das Topic, von dem der Reader liest (bei einer ContentFilteredTopic ihr Related Topic).

### §2.2.2.5.3.28 get_subscriber

**Spec:** §2.2.2.5.3.28, S. 96 — "This operation returns the Subscriber to which
the DataReader belongs."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_subscriber`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done

### §2.2.2.5.3.29 get_key_value (DataReader)

**Spec:** §2.2.2.5.3.29, S. 96 — "This operation can be used to retrieve the
instance key that corresponds to an instance_handle. The operation will only
fill the fields that form the key inside the key_holder instance. This operation
may return BAD_PARAMETER if the InstanceHandle_t a_handle does not correspond to
an existing data-object known to the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_key_value`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_get_key_value_returns_keyonly_sample`,
`crates/dcps/src/subscriber.rs::tests::get_key_value_uses_holder_decoder_for_writer_and_reader`

**Status:** done — ein unbekannter Handle liefert `BadParameter`.

### §2.2.2.5.3.29 get_key_value (DataReader) — UNSUPPORTED

**Spec:** §2.2.2.1.1, S. 17 — UNSUPPORTED "Can only be returned by operations
that are optional."; §2.2.2.5.3.29, S. 96 nennt nur BAD_PARAMETER.

**Repo:** —

**Tests:** `crates/dcps/tests/keyless_instance.rs::get_key_value_answers_for_the_one_instance_of_a_keyless_topic`

**Status:** `done` — wie beim DataWriter, auch im C-API; der Reader eines Topics ohne Schlüssel antwortet mit dem zuletzt empfangenen Sample.

### §2.2.2.5.3.30 lookup_instance (DataReader)

**Spec:** §2.2.2.5.3.30, S. 96–97 — "This operation takes as a parameter an
instance and returns a handle that can be used in subsequent operations … If the
instance has not been previously registered, or if for any other reason the
Service is unable to provide an instance handle, the Service will return the
special value HANDLE_NIL."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::lookup_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lookup_instance_after_take_with_info`

**Status:** done

### §2.2.2.5.3.31 delete_contained_entities (DataReader)

**Spec:** §2.2.2.5.3.31, S. 97 — "it deletes all contained ReadCondition and
QueryCondition objects. The operation will return PRECONDITION_NOT_MET if the
any of the contained entities is in a state where it cannot be deleted."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::delete_contained_entities`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_contained_entities_deletes_every_condition_the_reader_made`

**Status:** done — eine Condition hat keinen Zustand, der ihr Löschen
verhindert.

### §2.2.2.5.3.32 wait_for_historical_data

**Spec:** §2.2.2.5.3.32, S. 97 — "This operation is intended only for DataReader
entities that have a non-VOLATILE PERSISTENCE QoS kind … The operation
wait_for_historical_data blocks the calling thread until either all
“historical” data is received, or else the duration specified by the max_wait
parameter elapses … TIMEOUT indicates that max_wait elapsed before all the data
was received."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::wait_for_historical_data`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_reader_historical_data_received`)

**Tests:** `crates/dcps/tests/historical_data.rs::a_late_transient_local_reader_waits_for_the_history_and_then_holds_it`,
`crates/dcps/tests/historical_data.rs::a_volatile_reader_expects_no_history`,
`crates/dcps/src/runtime.rs::tests::wait_for_historical_data_times_out_for_a_writer_without_a_heartbeat`

**Status:** done — das Warten bis zur Historie, die sofortige Rückkehr eines VOLATILE-Readers und eines ohne zugeordneten Writer und das TIMEOUT (nächstes Item) sind getestet.

### §2.2.2.5.3.32 wait_for_historical_data — TIMEOUT

**Spec:** §2.2.2.5.3.32, S. 97 — "a return value of TIMEOUT indicates that
max_wait elapsed before all the data was received."

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::wait_for_historical_data_times_out_for_a_writer_without_a_heartbeat`

**Status:** done — ein entfernter TRANSIENT_LOCAL-, RELIABLE-Writer, der über SPDP und SEDP angekündigt wird (Datagramme eines von Hand gebauten SEDP-Stacks) und keinen HEARTBEAT sendet, hält das Warten offen; `Err(Timeout)` nach `max_wait`.

### §2.2.2.5.3.33 get_matched_publication_data

**Spec:** §2.2.2.5.3.33, S. 97 — "This operation retrieves information on a
publication that is currently “associated” with the DataReader … The
publication_handle must correspond to a publication currently associated with
the DataReader otherwise the operation will fail and return BAD_PARAMETER."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::matched_publication_data`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`,
`crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — die zugehörigen Einzelpunkte sind erledigt; siehe ihre Tests.

### §2.2.2.5.3.33 get_matched_publication_data — BAD_PARAMETER

**Spec:** §2.2.2.5.3.33, S. 97 — "otherwise the operation will fail and return
BAD_PARAMETER."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — der Test deckt es ab.

### §2.2.2.5.3.34 get_matched_publications

**Spec:** §2.2.2.5.3.34, S. 97 — "This operation retrieves the list of
publications currently “associated” with the DataReader … These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSPublications” builtin topic".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::matched_publications`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_reader_matched_publication_handles`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::publication_handle_matches_get_matched_publications`,
`crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`, `crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches_over_the_wire`

**Status:** done — die Handles der Liste treffen `SampleInfo::publication_handle` der Daten (Test) und den `instance_handle` im SampleInfo des DCPSPublication-Readers (in einem Prozess und über das Netz).

### §2.2.2.5.4 DataSample

**Spec:** §2.2.2.5.4, S. 98 — "A DataSample represents an atom of data
information (i.e., one value for one instance) as returned by DataReader’s
read/take operations. It consists of two parts: A SampleInfo and the Data."

**Repo:** `crates/dcps/src/sample.rs` (`Sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done

### §2.2.2.5.5 SampleInfo.sample_state

**Spec:** §2.2.2.5.5, S. 98 — "The sample_state (READ or NOT_READ) - indicates
whether or not the corresponding data sample has already been read."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::sample_state`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`

**Status:** done

### §2.2.2.5.5 SampleInfo.view_state

**Spec:** §2.2.2.5.5, S. 98 — "The view_state, (NEW, or NOT_NEW) - indicates
whether the DataReader has already seen samples for the most-current generation
of the related instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::view_state`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** done — Zeitpunkt der Momentaufnahme siehe §2.2.2.5.1.8.

### §2.2.2.5.5 SampleInfo.instance_state

**Spec:** §2.2.2.5.5, S. 98 — "The instance_state (ALIVE, NOT_ALIVE_DISPOSED, or
NOT_ALIVE_NO_WRITERS) - indicates whether the instance is currently in
existence or, if it has been disposed, the reason why it was disposed."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::instance_state`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`

**Status:** done — Zeitpunkt der Momentaufnahme siehe §2.2.2.5.1.3.

### §2.2.2.5.5 SampleInfo.disposed_generation_count

**Spec:** §2.2.2.5.5, S. 98 — "The disposed_generation_count that indicates the
number of times the instance had become alive after it was disposed explicitly
by a DataWriter, at the time the sample was received."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::disposed_generation_count`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.5.5 SampleInfo.no_writers_generation_count

**Spec:** §2.2.2.5.5, S. 98 — "The no_writers_generation_count that indicates
the number of times the instance had become alive after it was disposed because
there were no writers, at the time the sample was received."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::no_writers_generation_count`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — das Feld ist vorhanden; dass der Zähler steigt, prüft kein
Test (§2.2.2.5.1.5).

### §2.2.2.5.5 SampleInfo.sample_rank, generation_rank, absolute_generation_rank

**Spec:** §2.2.2.5.5, S. 99 — "The sample_rank that indicates the number of
samples related to the same instance that follow in the collection returned by
read or take. The generation_rank … The absolute_generation_rank …".

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::sample_rank`,
`SampleInfo::generation_rank`, `SampleInfo::absolute_generation_rank`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_generation_rank_starts_zero`,
`crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — die Ränge sind in der Rust-API gefüllt (§2.2.2.5.1.6).

### §2.2.2.5.5 SampleInfo.source_timestamp

**Spec:** §2.2.2.5.5, S. 99 — "the source_timestamp that indicates the time
provided by the DataWriter when the sample was written."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::source_timestamp`);
`crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::write_w_timestamp_stamps_the_sample_with_the_given_timestamp_not_the_clock`,
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`

**Status:** `done` — die Zeit des Writers, für Datensamples und
Lebenszyklus-Marken.

### §2.2.2.5.5 SampleInfo.source_timestamp — dispose und unregister

**Spec:** §2.2.2.4.2.13–§2.2.2.4.2.14, S. 66 — dispose und dispose_w_timestamp
liefern den source_timestamp "that is made available to DataReader objects by
means of the source_timestamp attribute inside the SampleInfo".

**Repo:** `crates/dcps/src/subscriber.rs` (`StagedIngest::Lifecycle`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_marker_names_its_writer_and_its_source_time`

**Status:** `done` — die Quellzeit der Marke aus `UserSample::Lifecycle` kommt
in die SampleInfo.

### §2.2.2.5.5 SampleInfo.instance_handle

**Spec:** §2.2.2.5.5, S. 99 — "the instance_handle that identifies locally the
corresponding instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::instance_handle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lookup_instance_after_take_with_info`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_filters_to_one_handle`

**Status:** done

### §2.2.2.5.5 SampleInfo.publication_handle

**Spec:** §2.2.2.5.5, S. 99 — "the publication_handle that identifies locally
the DataWriter that modified the instance. The publication_handle is the same
InstanceHandle_t that is returned by the operation get_matched_publications on
the DataReader and can also be used as a parameter to the DataReader operation
get_matched_publication_data."

**Repo:** `crates/dcps/src/subscriber.rs` (`writer_publication_handle`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/publication_handle_live.rs::publication_handle_matches_get_matched_publications`,
`crates/dcps/tests/publication_handle_live.rs::publication_handle_distinguishes_two_writers`,
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`

**Status:** `done` — für Datensamples und Lebenszyklus-Marken der Handle des
Writers.

### §2.2.2.5.5 SampleInfo.publication_handle — dispose und unregister

**Spec:** §2.2.2.5.5, S. 99 — "the DataWriter that modified the instance".

**Repo:** `crates/dcps/src/subscriber.rs` (`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_marker_names_its_writer_and_its_source_time`

**Status:** `done` — die Marke nennt den Writer, der sie sandte.

### §2.2.2.5.5 SampleInfo.valid_data

**Spec:** §2.2.2.5.5, S. 99 — "the valid_data flag that indicates whether the
DataSample contains data or else it is only used to communicate of a change in
the instance_state of the instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::valid_data`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_dispose_marker_has_invalid_data`

**Status:** done

### §2.2.2.5.6 SubscriberListener — on_data_on_readers

**Spec:** §2.2.2.5.6, S. 99 — Operation `on_data_on_readers(the_subscriber:
Subscriber)`; "the associated listener should be of concrete type
SubscriberListener."

**Repo:** `crates/dcps/src/listener.rs` (`SubscriberListener::on_data_on_readers`)

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`,
`crates/dcps/tests/data_available_live.rs::a_subscriber_listener_hears_data_on_its_readers`

**Status:** done — der Callback erhält den Handle des Subscribers
(§2.2.4.3).

### §2.2.2.5.7 DataReaderListener

**Spec:** §2.2.2.5.7, S. 100 — Operationen on_data_available,
on_sample_rejected, on_liveliness_changed, on_requested_deadline_missed,
on_requested_incompatible_qos, on_subscription_matched, on_sample_lost; "Some
implementations of the service may not propagate this information. In that case
the DDS specification does not require this listener operation
[on_subscription_matched] to be called."

**Repo:** `crates/dcps/src/listener.rs` (`DataReaderListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datareader_listener_is_object_safe`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_available_at_reader`,
`crates/dcps/tests/listener_trigger_c22c.rs::sample_rejected_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::sample_lost_dispatcher_fires_listener`,
`crates/dcps/tests/listeners_fire_without_polling.rs::matched_listeners_fire_on_match_and_unmatch_without_a_poll`

**Status:** done — alle sieben, `on_subscription_matched` eingeschlossen; die
Callbacks erhalten den Handle des Readers (§2.2.4.3).

### §2.2.2.5.8 ReadCondition — an einen DataReader gebunden, Masken

**Spec:** §2.2.2.5.8, S. 100–101 — "ReadCondition objects are conditions
specifically dedicated to read operations and attached to one DataReader …
ReadCondition objects allow an application to specify the data samples it is
interested in (by specifying the desired sample-states, view-states, and
instance-states) … More than one ReadCondition may be attached to the same
DataReader."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition`);
`crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`

**Status:** done

### §2.2.2.5.8.1 get_datareader

**Spec:** §2.2.2.5.8.1, S. 101 — "This operation returns the DataReader
associated with the ReadCondition. Note that there is exactly one DataReader
associated with each ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_datareader`)

**Tests:** `crates/dcps/tests/status_condition.rs::a_read_condition_names_its_reader`

**Status:** done — `get_datareader` liefert den Reader der Condition.

### §2.2.2.5.8.2 get_sample_state_mask

**Spec:** §2.2.2.5.8.2, S. 101 — "This operation returns the set of
sample-states that are taken into account to determine the trigger_value of the
ReadCondition. These are the sample-states specified when the ReadCondition was
created."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_sample_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done — die Bits folgen `sample_info::sample_state_mask` (siehe
§2.2.2.5.1.9).

### §2.2.2.5.8.3 get_view_state_mask

**Spec:** §2.2.2.5.8.3, S. 101 — "This operation returns the set of view-states
that are taken into account to determine the trigger_value of the
ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_view_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.5.8.4 get_instance_state_mask

**Spec:** §2.2.2.5.8.4, S. 101 — "This operation returns the set of
instance-states that are taken into account to determine the trigger_value of
the ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_instance_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.5.9 QueryCondition — Filter auf lokale Daten

**Spec:** §2.2.2.5.9, S. 101–102 — "QueryCondition objects are specialized
ReadCondition objects that allow the application to also specify a filter on the
locally available data. The query (query_expression) is similar to an SQL WHERE
clause and can be parameterized by arguments that are dynamically changeable by
the set_query_parameters operation … This feature is optional."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition`,
`QueryCondition::evaluate`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_evaluate_filters_per_sample`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`

**Status:** done

### §2.2.2.5.9.1 get_query_expression

**Spec:** §2.2.2.5.9.1, S. 102 — "This operation returns the query_expression
associated with the QueryCondition. That is, the expression specified when the
QueryCondition was created."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::get_query_expression`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_inherits_trigger_from_base`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`

**Status:** done

### §2.2.2.5.9.2 get_query_parameters

**Spec:** §2.2.2.5.9.2, S. 102 — "the parameters specified on the last
successful call to set_query_parameters, or if set_query_parameters was never
called, the arguments specified when the QueryCondition was created."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::get_query_parameters`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_set_query_parameters_roundtrip`

**Status:** done

### §2.2.2.5.9.3 set_query_parameters

**Spec:** §2.2.2.5.9.3, S. 102 — "This operation changes the query_parameters
associated with the QueryCondition."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::set_query_parameters`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_set_query_parameters_roundtrip`

**Status:** done

---

## §2.2.3 Supported QoS

### §2.2.3 QoS als Liste von QosPolicy-Objekten

**Spec:** §2.2.3, S. 102–103 — "QoS is comprised of individual QoS policies
(objects of type deriving from QosPolicy) … QoS (i.e., a list of QosPolicy
objects) may be associated with all Entity objects in the system such as Topic,
DataWriter, DataReader, Publisher, Subscriber, and DomainParticipant." Figure
2.12 zeigt die 22 Policies mit ihren Feldern.

**Repo:** `crates/qos/src/policies/` (ein Modul je Policy);
`crates/dcps/src/qos.rs` (QoS-Sätze je Entity)

**Tests:** `crates/qos/src/pid.rs::tests::every_qos_policy_has_a_distinct_identifier`,
`crates/qos/src/policies/qos_set.rs::tests::defaults_are_compatible`

**Status:** done — die Policies und ihre Felder sind unten je Policy geführt.

### §2.2.3 Konsistenz bei set_qos

**Spec:** §2.2.3, S. 104 — "Some QosPolicy values may not be consistent with
other ones … When a set of QosPolicy is passed (set_qos operations), the set
resulting from adding the new policies on top of the previous is checked for
consistency. If the resulting QoS is inconsistent, the change of QoS operation
fails and the previous values are retained."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — `set_qos` und `create_*` prüfen die Konsistenz; ein
abgelehntes `set_qos` lässt die vorherigen Werte stehen.

### §2.2.3 Requested/Offered — Unverträglichkeit verhindert die Kommunikation

**Spec:** §2.2.3, S. 104 — "If the two policies are not compatible, the Service
will not establish communications between the two Entity objects and will record
this fact by means of the OFFERED_INCOMPATIBLE_QOS on the publisher end and
REQUESTED_INCOMPATIBLE_QOS on the subscriber end".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`RxoView`, Zuordnungsstellen mit `record_match_refusal`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_same_runtime_pair_is_refused_for_an_rxo_policy_and_counted_once`,
`crates/dcps/src/runtime.rs::tests::an_incompatible_remote_endpoint_is_counted_once`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.3 RxO-Eigenschaft Yes / No / N/A

**Spec:** §2.2.3, S. 104 — "An ‘RxO’ setting of “Yes” indicates that the policy
can be set both at the publishing and subscribing ends and the values must be
set in a compatible manner … “No” … all combinations of values are compatible …
“N/A” indicates that the policy can only be specified at either the publishing
or the subscribing end".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView`: DURABILITY, RELIABILITY,
DEADLINE, LIVELINESS, LATENCY_BUDGET, DESTINATION_ORDER, OWNERSHIP,
PRESENTATION)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_requested_both_compatible_matches`,
`crates/qos/src/compatibility.rs::tests::compute_compatibility_reports_multiple_mismatches`

**Status:** done — die Zuordnung je Policy ist unten geführt.

### §2.2.3 Eigenschaft Changeable

**Spec:** §2.2.3, S. 104 — "a policy with ‘changeable’ setting of ‘NO” is
considered “immutable” and can only be specified either at Entity creation time
or else prior to calling the enable operation on the Entity."

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`

**Status:** done — die Zuordnung je Policy ist unten geführt.

### §2.2.3.1 USER_DATA — Feld value

**Spec:** §2.2.3, S. 104 — USER_DATA: "A sequence of octets: “value”" für
DomainParticipant, DataReader, DataWriter; "User data not known by the
middleware, but distributed by means of built-in topics".

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy::value`);
`crates/dcps/src/qos.rs` (`DomainParticipantQos::user_data`,
`DataWriterQos::user_data`, `DataReaderQos::user_data`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::user_data_roundtrip`,
`crates/dcps/tests/participant_config_user_data.rs::an_explicit_config_keeps_the_user_data_of_the_qos`,
`crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done — ein Wert über 64 KiB (`MAX_OPAQUE_LEN`) wird beim Lesen
abgewiesen; die Spec setzt keine Grenze.

### §2.2.3.1 USER_DATA — Default

**Spec:** §2.2.3, S. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::user_data_default_is_empty`

**Status:** done

### §2.2.3.1 USER_DATA — RxO No

**Spec:** §2.2.3, S. 104 — USER_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy` vergleicht
USER_DATA nicht)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.1 USER_DATA — Changeable Yes

**Spec:** §2.2.3, S. 104 — USER_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_qos`:
`update_participant_user_data`); `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs` (`runtime_qos_update`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.1 USER_DATA — Zweck

**Spec:** §2.2.3.1, S. 113 — "The purpose of this QoS is to allow the
application to attach additional information to the created Entity objects such
that when a remote application discovers their existence it can access that
information".

**Repo:** `crates/dcps/src/builtin_topics.rs` (`user_data` in
`ParticipantBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done

### §2.2.3.2 TOPIC_DATA — Feld value

**Spec:** §2.2.3, S. 104 — TOPIC_DATA: "A sequence of octets: “value”" für
Topic.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy`);
`crates/dcps/src/qos.rs` (`TopicQos::topic_data`; zusätzlich
`DataWriterQos::topic_data` und `DataReaderQos::topic_data` als Überschreibung
je Endpoint)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done — DataWriter und DataReader tragen das Feld zusätzlich, die
Spec ordnet es nur dem Topic zu.

### §2.2.3.2 TOPIC_DATA — Default

**Spec:** §2.2.3, S. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::topic_data_default_is_empty`

**Status:** done

### §2.2.3.2 TOPIC_DATA — RxO No

**Spec:** §2.2.3, S. 104 — TOPIC_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.2 TOPIC_DATA — Changeable Yes

**Spec:** §2.2.3, S. 104 — TOPIC_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/topic.rs` (`Topic::set_qos`:
`update_user_topic_data`, Neuankündigung)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`

**Status:** done

### §2.2.3.2 TOPIC_DATA — Zweck

**Spec:** §2.2.3.2, S. 114 — "The purpose of this QoS is to allow the
application to attach additional information to the created Topic such that when
a remote application discovers their existence it can examine the information".

**Repo:** `crates/dcps/src/builtin_topics.rs` (`topic_data` in
`TopicBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done

### §2.2.3.3 GROUP_DATA — Feld value

**Spec:** §2.2.3, S. 104 — GROUP_DATA: "A sequence of octets: “value”" für
Publisher, Subscriber.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy`);
`crates/dcps/src/qos.rs` (`PublisherQos::group_data`, `SubscriberQos::group_data`;
zusätzlich je Endpoint, `effective_group_data`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::group_data_large_payload`,
`crates/dcps/src/qos.rs::group_policy_tests::empty_endpoint_group_data_inherits_the_group_octets`

**Status:** done

### §2.2.3.3 GROUP_DATA — Default

**Spec:** §2.2.3, S. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::group_data_default_is_empty`

**Status:** done

### §2.2.3.3 GROUP_DATA — RxO No

**Spec:** §2.2.3, S. 104 — GROUP_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.3 GROUP_DATA — Changeable Yes

**Spec:** §2.2.3, S. 104 — GROUP_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`:
`update_user_group_qos`), `crates/dcps/src/subscriber.rs`
(`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`

**Status:** done — der Test ändert PARTITION; GROUP_DATA nimmt denselben Weg
(`update_user_group_qos`), eine eigene Prüfung der neuen Oktette gibt es
nicht.

### §2.2.3.3 GROUP_DATA — Zweck

**Spec:** §2.2.3.3, S. 114 — "The value of the GROUP_DATA is available to the
application on the DataReader and DataWriter entities and is propagated by
means of the built-in topics."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`group_data` in
`PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.4 DURABILITY — Feld kind

**Spec:** §2.2.3, S. 105 — DURABILITY: "A “kind”: VOLATILE, TRANSIENT_LOCAL,
TRANSIENT, or PERSISTENT" für Topic, DataReader, DataWriter.

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityKind`,
`DurabilityQosPolicy`); `crates/dcps/src/qos.rs`

**Tests:** `crates/qos/src/policies/durability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/durability.rs::tests::try_from_u32_is_strict`

**Status:** done

### §2.2.3.4 DURABILITY — Default VOLATILE

**Spec:** §2.2.3, S. 105 — VOLATILE "This is the default kind."

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityKind::default`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::default_is_volatile`,
`crates/qos/src/defaults.rs::tests::durability_default_volatile`

**Status:** done

### §2.2.3.4 DURABILITY — RxO: offered kind >= requested kind

**Spec:** §2.2.3.4, S. 114 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … VOLATILE < TRANSIENT_LOCAL < TRANSIENT <
PERSISTENT."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`);
`crates/qos/src/policies/durability.rs` (`DurabilityQosPolicy::is_compatible_with`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::compatibility_offered_ge_requested`,
`crates/qos/src/compatibility.rs::tests::durability_offered_ge_requested`,
`crates/dcps/tests/transient_local_durability.rs::linux::transient_local_reader_rejects_volatile_writer`

**Status:** done

### §2.2.3.4 DURABILITY — Changeable No

**Spec:** §2.2.3, S. 105 — DURABILITY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`

**Status:** done

### §2.2.3.4 DURABILITY — VOLATILE

**Spec:** §2.2.3, S. 105 — "The Service does not need to keep any samples of
data-instances on behalf of any DataReader that is not known by the DataWriter
at the time the instance is written."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::skip_samples_up_to`);
`crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::volatile_writer_does_not_deliver_history_to_late_joiner`,
`crates/dcps/src/runtime.rs::tests::qr_transient_local_late_join_replay_vs_volatile`

**Status:** done

### §2.2.3.4 DURABILITY — TRANSIENT_LOCAL

**Spec:** §2.2.3, S. 105 — "The Service will attempt to keep some samples so
that they can be delivered to any potential late-joining DataReader … For
TRANSIENT_LOCAL, the service is only required to keep the data in the memory of
the DataWriter that wrote the data".

**Repo:** `crates/dcps/src/runtime.rs` (Historie des Writers für späte Reader,
`enforce_retained_limits`)

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::transient_local_writer_delivers_history_to_late_joiner`,
`crates/dcps/tests/transient_local_history_depth.rs::keep_last_one_replays_only_the_newest_sample`,
`crates/dcps/src/runtime.rs::tests::transient_local_reader_acknacked_on_metatraffic_socket_gets_history_and_live_samples`

**Status:** done

### §2.2.3.4 DURABILITY — TRANSIENT [optional]

**Spec:** §2.2.3, S. 105 — "For TRANSIENT, the service is only required to keep
the data in memory and not in permanent storage; but the data is not tied to the
lifecycle of the DataWriter and will, in general, survive it. Support for
TRANSIENT kind is optional."

**Repo:** `crates/dcps/src/durability_service.rs` (`InMemoryDurabilityBackend`);
`crates/dcps/src/publisher.rs` (`DataWriter::build_durability_backend`)

**Tests:** `crates/dcps/tests/durability_service_qos.rs::transient_writer_has_in_memory_backend`,
`crates/dcps/tests/durability_service_qos.rs::transient_backend_outlives_writer_cache_keep_last_eviction`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_store.rs::the_store_outlives_the_writer_that_filled_it`, `crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`

**Status:** done — der Speicher überdauert die Verdrängung aus dem Cache des Writers und das Löschen des Writers, und ein Reader, der nach dem Löschen des letzten Anwendungs-DataWriters entsteht, bekommt die Samples (der fiktive DataWriter, siehe nächstes Item). Ein eigenständiger Dienst ist `crates/durability-service/`.

### §2.2.3.4 DURABILITY — TRANSIENT überdauert den DataWriter

**Spec:** §2.2.3, S. 105 — "the data is not tied to the lifecycle of the
DataWriter and will, in general, survive it."

**Repo:** `crates/dcps/src/participant.rs` (`ParticipantInner::durability_store`);
`crates/dcps/src/publisher.rs` (`Publisher::durability_store_for`)

**Tests:** `crates/dcps/tests/durability_store.rs::the_store_outlives_the_writer_that_filled_it`

**Status:** `done` — der Speicher gehört zum Topic des Participant; ein später erzeugter Writer findet, was der gelöschte schrieb, und setzt die Sequenznummern des Speichers fort (`next_sequence`, bei einem PERSISTENT-Speicher über dem Höchsten auf der Platte). Ein Reader, der erst nach dem Löschen aller Writer entsteht, bekommt nichts, solange kein Writer den Speicher wieder bedient: dafür fehlen die fiktiven Entities (nächstes Item).

### §2.2.3.4 DURABILITY — PERSISTENT [optional]

**Spec:** §2.2.3, S. 105 — "[optional] Data is kept on permanent storage, so
that they can outlive a system session."

**Repo:** `crates/dcps/src/durability_service.rs` (`OnDiskDurabilityBackend`)

**Tests:** `crates/dcps/src/durability_service.rs::tests::on_disk_persistent_survives_backend_drop`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::persistent_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::a_persistent_writer_without_a_store_is_refused`

**Status:** done

### §2.2.3.4 DURABILITY — Steuerung der Daten für späte Reader

**Spec:** §2.2.3.4, S. 114 — "This QoS policy controls whether the Service will
actually make data available to late-joining readers … the Service may choose
to maintain some data for its own purposes (e.g., flow control) and yet not make
it available to late-joining readers if the DURABILITY QoS policy is set to
VOLATILE."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::a_transient_local_writer_serves_a_volatile_late_joiner_nothing`,
`crates/dcps/tests/transient_local_durability.rs::linux::volatile_writer_does_not_deliver_history_to_late_joiner`

**Status:** done

### §2.2.3.4 DURABILITY — fiktiver Durability-DataReader und -DataWriter je Topic

**Spec:** §2.2.3.4, S. 114 — "the service behaves “as if” for each Topic that has
TRANSIENT or PERSISTENT DURABILITY kind there was a corresponding “built-in”
DataReader and DataWriter … For each Topic, the built-in fictitious
“persistence service” DataReader and DataWriter has its QoS configured from the
Topic QoS of the corresponding Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`retain_user_writer_as_service`, `release_service_writer`); `crates/dcps/src/publisher.rs` (`WriterInner::delete`)

**Tests:** `crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`, `crates/dcps/tests/durability_service_entities.rs::the_service_writer_ends_with_the_content_of_the_store`,
`crates/dcps/tests/durability_store.rs::the_store_follows_the_topics_durability_service`, `crates/dcps/tests/durability_store.rs::a_default_topic_leaves_the_setting_to_the_writer`, `crates/dcps/tests/durability_store.rs::a_writer_that_differs_from_the_topic_is_overridden`, `crates/durability-service/tests/keyed_topic_end_to_end.rs`, `crates/durability-service/tests/writer_dies.rs::late_joiner_gets_history_after_writer_dies`, `crates/durability-service/tests/restart.rs`, `crates/durability-service/tests/cross_vendor_ingest.rs`, `crates/durability-service/tests/discovered_durability_service_qos.rs`

**Status:** done — die fiktiven Entities sind an zwei Orten verwirklicht, die zusammen die Klausel abdecken. Im Participant: der Speicher je Topic, der fiktive DataWriter (der Endpoint des letzten Anwendungs-DataWriters, gehalten, solange der Speicher Samples hält) und die Prüfungen gegen die Topic-QoS. Der Speicher folgt der DURABILITY_SERVICE des Topics (§2.2.3.5: sie konfiguriert die fiktiven Entities, die je Topic bestehen); lässt ein Topic sie auf dem Default, entscheidet der erste Writer, der den Speicher anlegt; ein Writer, der von einem nicht voreingestellten Topic abweicht, wird überstimmt und `writer.durability_service.overridden_by_topic` wird gemeldet. Die Samples entfernter Writer nimmt der Participant nicht auf; das ist der domänenweite Dienst `crates/durability-service/` (ADR 0009): er nimmt mit einem RELIABLE-, TRANSIENT_LOCAL-, KEEP_ALL-Reader jedes Sample jedes entdeckten Writers auf, ob lokal, entfernt oder von einem anderen Hersteller, speichert es (sqlite, Dateien oder Speicher) und gibt es mit einem TRANSIENT_LOCAL-Writer wieder, auch nach dem Ende des ursprünglichen Writers und nach seinem eigenen Neustart. Den Aufbewahrungsvertrag nimmt er aus der DURABILITY_SERVICE, die der Writer ankündigt.

### §2.2.3.4 DURABILITY — Unverträglichkeit mit den fiktiven Entities

**Spec:** §2.2.3.4, S. 114–115 — "a DataWriter that has an incompatible QoS with
respect to what the Topic specified will not send its data to the
transient/persistent service … Incompatibilities between local DataReader/
DataWriter entities and the corresponding fictitious “built-in
transient/persistent entities” cause the REQUESTED_INCOMPATIBLE_QOS/
OFFERED_INCOMPATIBLE_QOS status to change".

**Repo:** `crates/dcps/src/runtime.rs` (`check_writer_against_fictitious_reader`, `check_reader_against_fictitious_writer`, `RxoView::of_topic`)

**Tests:** `crates/dcps/tests/durability_service_entities.rs::an_endpoint_incompatible_with_the_topic_changes_its_status`, `crates/dcps/tests/durability_service_entities.rs::an_incompatible_writer_does_not_feed_the_store`

**Status:** done — `check_writer_against_fictitious_reader` und `check_reader_against_fictitious_writer` vergleichen einen Endpoint beim `enable` mit der Topic-QoS eines TRANSIENT- oder PERSISTENT-Topics: ein unverträglicher Writer zählt in OFFERED_INCOMPATIBLE_QOS und sendet nichts an den Speicher, ein unverträglicher Reader zählt in REQUESTED_INCOMPATIBLE_QOS. Ein VOLATILE-Writer auf einem solchen Topic ist unverträglich (DURABILITY), wie die Klausel es liest. Verglichen wird beim `enable`; eine spätere Änderung einer änderbaren Topic-Policy wird nicht neu verglichen.

### §2.2.3.4 DURABILITY — service_cleanup_delay: wann der Dienst eine Instanz entfernt

**Spec:** §2.2.3.4, S. 115 — "Information on a data-instances is maintained
until the following conditions are met: 1. the instance has been explicitly
disposed … 2. … there are no more “live” DataWriter entities writing the
instance … 3. a time interval longer that service_cleanup_delay has elapsed".

**Repo:** `crates/dcps/src/durability_service.rs` (`LiveStoreWriters`); `crates/dcps/src/publisher.rs` (`WriterInner::leave_store`, `WriterStatus::leave_store_on_lost_liveliness`)

**Tests:** `crates/dcps/tests/durability_store.rs::a_disposed_unregistered_instance_stays_for_the_service_cleanup_delay`,
`crates/dcps/tests/durability_store.rs::an_instance_that_is_not_disposed_stays`,
`crates/dcps/src/durability_service.rs::tests::replay_applies_the_service_cleanup_delay`,
`crates/dcps/tests/durability_store.rs::the_cleanup_delay_waits_for_the_last_writer_of_the_instance`, `crates/dcps/tests/durability_store.rs::a_deleted_writer_is_no_live_writer`, `crates/dcps/tests/durability_service_entities.rs::a_writer_that_lost_its_liveliness_is_no_live_writer`

**Status:** done — die drei Bedingungen: die Instanz ist verworfen, kein lebender Writer des Participant schreibt sie (`LiveStoreWriters`: ein Writer, der abmeldet, gelöscht wird oder seine Liveliness verliert, ist keiner mehr; ein späteres Write macht ihn wieder zu einem) und `service_cleanup_delay` ist abgelaufen. Writer anderer Participants sieht der Speicher nicht.

### §2.2.3.5 DURABILITY_SERVICE — Felder

**Spec:** §2.2.3, S. 105–106 — DURABILITY_SERVICE für Topic, DataWriter:
`service_cleanup_delay` (Dauer), `history_kind`, `history_depth`,
`max_samples`, `max_instances`, `max_samples_per_instance`.

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy`); `crates/dcps/src/qos.rs`
(`TopicQos::durability_service`, `DataWriterQos::durability_service`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::roundtrip`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — Defaults

**Spec:** §2.2.3, S. 106 — "service_cleanup_delay … By default, zero …
history_kind=KEEP_LAST history_depth=1 … max_samples, max_instances,
max_samples_per_instance … By default they are all LENGTH_UNLIMITED."

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy::default`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::default_matches_spec`,
`crates/qos/src/defaults.rs::tests::durability_service_defaults_match_spec`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — RxO No

**Spec:** §2.2.3, S. 105 — DURABILITY_SERVICE: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` ohne DURABILITY_SERVICE)

**Tests:** `crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — Changeable No

**Spec:** §2.2.3, S. 105 — DURABILITY_SERVICE: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.5 DURABILITY_SERVICE — HISTORY und RESOURCE_LIMITS des Dienstes

**Spec:** §2.2.3.5, S. 115 — "This policy is used to configure the HISTORY QoS
and the RESOURCE_LIMITS QoS used by the fictitious DataReader and DataWriter used
by the “persistence service.”"

**Repo:** `crates/dcps/src/durability_service.rs` (`InMemoryDurabilityBackend`,
`OnDiskDurabilityBackend`: Tiefe und Grenzen aus DURABILITY_SERVICE)

**Tests:** `crates/dcps/src/durability_service.rs::tests::in_memory_keeplast_caps_history_at_depth`,
`crates/dcps/src/durability_service.rs::tests::in_memory_max_instances_returns_oor`,
`crates/dcps/src/durability_service.rs::tests::on_disk_max_samples_per_instance_returns_oor`

**Status:** done — angewandt auf den Speicher des Writers (siehe §2.2.3.4).

### §2.2.3.6 PRESENTATION — Feld access_scope

**Spec:** §2.2.3, S. 106 — PRESENTATION für Publisher, Subscriber: "An
“access_scope”: INSTANCE, TOPIC, GROUP" — INSTANCE "Scope spans only a single
instance", TOPIC "Scope spans to all instances within the same DataWriter (or
DataReader)", GROUP "[optional] Scope spans to all instances belonging to
DataWriter (or DataReader) entities within the same Publisher (or
Subscriber)".

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationAccessScope`);
`crates/dcps/src/qos.rs` (`PublisherQos::presentation`,
`SubscriberQos::presentation`; Überschreibung je Endpoint,
`effective_presentation`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/dcps/src/qos.rs::group_policy_tests::endpoint_default_presentation_inherits_the_group_value`

**Status:** done

### §2.2.3.6 PRESENTATION — Felder coherent_access und ordered_access

**Spec:** §2.2.3, S. 106–107 — "coherent_access … the ability to group a set of
changes as a unit on the publishing end such that they are received as a unit at
the subscribing end … ordered_access … the ability of the subscriber to see
changes in the same order as they occurred on the publishing end."

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/dcps/src/runtime.rs::tests::publisher_coherent_and_ordered_flags_reach_the_writer_announcement`

**Status:** done

### §2.2.3.6 PRESENTATION — Defaults

**Spec:** §2.2.3, S. 106–107 — INSTANCE "This is the default access_scope …
The default setting of coherent_access is FALSE … The default setting of
ordered_access is FALSE."

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy::default`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::default_instance_no_flags`,
`crates/qos/src/defaults.rs::tests::presentation_default_instance_no_flags`

**Status:** done

### §2.2.3.6 PRESENTATION — RxO

**Spec:** §2.2.3.6, S. 116 — "1. The inequality “offered access_scope >=
requested access_scope” … INSTANCE < TOPIC < GROUP. 2. Requested
coherent_access is FALSE, or else both offered and requested coherent_access are
TRUE. 3. Requested ordered_access is FALSE, or else both offered and requested
ordered _access are TRUE."

**Repo:** `crates/qos/src/policies/presentation.rs` (`is_compatible_with`);
`crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_coherent_access_unoffered_rejects_on_both_sides`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_ordered_access_unoffered_rejects_on_both_sides`,
`crates/qos/src/compatibility.rs::tests::presentation_scope_and_flags`

**Status:** done

### §2.2.3.6 PRESENTATION — Changeable No

**Spec:** §2.2.3, S. 106 — PRESENTATION: Changeable "No".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`),
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`

**Status:** done

### §2.2.3.6 PRESENTATION — coherent_access mit INSTANCE

**Spec:** §2.2.3.6, S. 115 — "If access_scope is set to INSTANCE, the use of
begin_coherent_change and end_coherent_change has no effect on how the
subscriber can access the data because with the scope limited to each instance,
changes to separate instances are considered independent and thus cannot be
grouped by a coherent change."

**Repo:** `crates/dcps/src/subscriber.rs::DataReader::coherent_access_active`;
`crates/dcps/src/runtime.rs` (`PresentationInstanceCoherence`, `dcps.presentation_instance_coherence`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_coherent_reader_still_waits_for_end_coherent_changes`,
`crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_with_no_coherence_presents_the_samples_as_they_arrive`,
`crates/dcps/tests/runtime_config_layering.rs::the_presentation_instance_coherence_setting_takes_both_values_and_refuses_others`

**Status:** `done` — die Spec widerspricht sich: die Tabelle (S. 106) „order and coherent changes apply to each instance separately“ gegen §2.2.3.6 (S. 115) „has no effect on how the subscriber can access the data“; beide sagen, dass keine Kohärenz über Instanzen hinweg besteht, sie unterscheiden sich für einen kohärenten Satz innerhalb einer Instanz. Entscheidung: Default ist die Tabellen-Lesart (`per_instance`, der Satz wird bis zu seinem Ende gehalten), weil die OMG-Suite dds-rtps (`Test_CoherentSets_10`, `_19`) genau das gegen die anderen Hersteller prüft. Die Lesart des Textes ist die Einstellung `dcps.presentation_instance_coherence: none` (Umgebung `ZERODDS_PRESENTATION_INSTANCE_COHERENCE`): unter INSTANCE wird der Satz nicht gehalten.

### §2.2.3.6 PRESENTATION — coherent_access mit TOPIC

**Spec:** §2.2.3.6, S. 115 — "If access_scope is set to TOPIC, then coherent
changes … will be made available as such to each remote DataReader
independently. That is, changes made to instances within each individual
DataWriter will be available as coherent with respect to other changes to
instances in that same DataWriter".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::coherent_gate`,
`DataReader::release_coherent_set`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::two_coherent_sets_are_released_one_at_a_time`,
`crates/dcps/src/subscriber.rs::tests::every_end_of_coherent_set_form_releases_the_writers_set`,
`crates/dcps/src/subscriber.rs::tests::another_writers_end_leaves_a_set_held`

**Status:** done

### §2.2.3.6 PRESENTATION — coherent_access mit GROUP

**Spec:** §2.2.3.6, S. 115 — "If access_scope is set to GROUP, then coherent
changes made to instances through a DataWriter attached to a common Publisher are
made available as a unit to remote subscribers."

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/publisher.rs`
(`Publisher::end_coherent_changes`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/src/group_order.rs::tests::the_group_coherent_set_of_table_9_23_goes_out_whole_after_its_end`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_in_a_group_coherent_set_waits_for_its_end_over_udp`

**Status:** done — gebaut auf `wip/omg-rc7`, Stand in
`internal/rtps/group-access-followup.md`.

### §2.2.3.6 PRESENTATION — ordered_access mit INSTANCE

**Spec:** §2.2.3.6, S. 116 — "If access_scope is set to INSTANCE (the lowest
level), then changes to each instance are considered unordered relative to
changes to any other instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::instance_scope_ordering_active`,
`DataReader::present_by_access_scope`)

**Tests:** `crates/dcps/tests/ordered_access_e2e.rs::ordered_access_under_instance_scope_is_inert`,
`crates/dcps/tests/ordered_access_e2e.rs::instance_scope_ordered_access_groups_the_plain_take_by_instance`

**Status:** done

### §2.2.3.6 PRESENTATION — ordered_access mit TOPIC

**Spec:** §2.2.3.6, S. 116 — "If access_scope is set to TOPIC, changes …
made by a single DataWriter are made available to subscribers in the same order
they occur."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_ordered_access_keeps_write_order_on_the_plain_take`

**Status:** `done`

### §2.2.3.6 PRESENTATION — ordered_access mit GROUP

**Spec:** §2.2.3.6, S. 116 — "if access_scope is set to GROUP, changes made to
instances via DataWriter entities attached to the same Publisher object are made
available to subscribers on the same order they occur."

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/subscriber.rs`
(`Subscriber::get_datareaders`, `DataReader::presentation_limit`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_takes_its_place_in_the_group_order_same_runtime`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — Übergabe in GSN-Reihenfolge je Paar aus Writer und Reader, und `get_datareaders` ordnet nach der GSN.

### §2.2.3.7 DEADLINE — Feld period

**Spec:** §2.2.3, S. 107 — DEADLINE für Topic, DataReader, DataWriter: "A
duration “period”".

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy::period`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::roundtrip`

**Status:** done

### §2.2.3.7 DEADLINE — Default unendlich

**Spec:** §2.2.3, S. 107 — "The default value of the deadline period is
infinite."

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy::default`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::default_is_infinite`,
`crates/dcps/tests/deadline_qos.rs::deadline_default_is_infinite_so_counter_stays_zero`

**Status:** done

### §2.2.3.7 DEADLINE — RxO: offered period <= requested period

**Spec:** §2.2.3.7, S. 116 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered deadline period <=
requested deadline period” evaluates to ‘TRUE.’"

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`deadline_compat`)

**Tests:** `crates/qos/src/compatibility.rs::tests::deadline_offered_le_requested`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done — `qos_duration_to_nanos` bildet INFINITE und die Dauer 0
auf denselben Wert 0 ab; eine angeforderte Periode 0 gilt dadurch als
unendlich. Gelesen, nicht gemessen; siehe Befunddatei.

### §2.2.3.7 DEADLINE — Changeable Yes

**Spec:** §2.2.3, S. 107 — DEADLINE: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`
(`runtime_qos_update`: `deadline`); `crates/dcps/src/topic.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done

### §2.2.3.7 DEADLINE — Vertrag und Überwachung

**Spec:** §2.2.3, S. 107; §2.2.3.7, S. 116 — "DataReader expects a new sample
updating the value of each instance at least once every deadline period.
DataWriter indicates that the application commits to write a new value … for
each instance … the fulfillment of this contract is monitored by the Service and
the application is informed of any violations".

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::writes_within_deadline_keep_counter_at_zero`

**Status:** done

### §2.2.3.7 DEADLINE — Konsistenz mit TIME_BASED_FILTER

**Spec:** §2.2.3, S. 107; §2.2.3.7, S. 116 — "It is inconsistent for a DataReader
to have a DEADLINE period less than its TIME_BASED_FILTER’s minimum_separation …
the settings must be such that “deadline period>= minimum_separation.”"

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::a_time_based_filter_longer_than_the_deadline_refuses_the_reader`

**Status:** `done` — `create_datareader` und `DataReader::set_qos` lehnen ein
`minimum_separation` über der DEADLINE-Periode ab.

### §2.2.3.8 LATENCY_BUDGET — Feld duration

**Spec:** §2.2.3, S. 107 — LATENCY_BUDGET für Topic, DataReader, DataWriter: "A
duration “duration”" — "Specifies the maximum acceptable delay from the time the
data is written until the data is inserted in the receiver's application-cache
and the receiving application is notified of the fact."

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::roundtrip`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — Default 0

**Spec:** §2.2.3, S. 107 — "The default value of the duration is zero
indicating that the delay should be minimized."

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy::default`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::default_is_zero`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — RxO: offered duration <= requested duration

**Spec:** §2.2.3.8, S. 117 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered duration <= requested
duration” evaluates to ‘TRUE.’"

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`is_compatible_with`);
`crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_offered_latency_budget_incompatible_rejects_match`,
`crates/dcps/src/runtime.rs::tests::rxo_requested_latency_budget_incompatible_rejects_match`,
`crates/qos/src/compatibility.rs::tests::latency_budget_offered_le_requested`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — Changeable Yes

**Spec:** §2.2.3, S. 107 — LATENCY_BUDGET: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`
(`runtime_qos_update`: `latency_budget`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_qos_change_is_decided_again_against_a_matched_reader`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — Hinweis

**Spec:** §2.2.3, S. 107; §2.2.3.8, S. 116–117 — "This policy is a hint to the
Service, not something that must be monitored or enforced. The Service is not
required to track or alert the user of any violation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — eine Erlaubnis; ZeroDDS nutzt den Wert nur
im Vergleich (RxO).

### §2.2.3.9 OWNERSHIP — Feld kind

**Spec:** §2.2.3, S. 107 — OWNERSHIP für Topic, DataReader, DataWriter: "A
“kind” SHARED EXCLUSIVE" — "[optional] Specifies whether it is allowed for
multiple DataWriters to write the same instance of the data and if so, how these
modifications should be arbitrated".

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipKind`,
`OwnershipQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::roundtrip`,
`crates/qos/src/policies/ownership.rs::tests::try_from_u32_valid_variants`

**Status:** done

### §2.2.3.9 OWNERSHIP — Default SHARED

**Spec:** §2.2.3, S. 107 — SHARED "This is the default behavior if the OWNERSHIP
QoS policy is not specified or supported."

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipKind::default`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::default_shared`,
`crates/qos/src/defaults.rs::tests::ownership_default_shared`

**Status:** done

### §2.2.3.9 OWNERSHIP — RxO: gleiche Art

**Spec:** §2.2.3.9.2, S. 118 — "The value of the OWNERSHIP kind offered must
exactly match the one requested or else they are considered incompatible."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::ownership_must_match_exactly`,
`crates/qos/src/policies/qos_set.rs::tests::ownership_mismatch_reported`

**Status:** done — die Tests prüfen `OwnershipQosPolicy::is_compatible_with`;
`first_incompatible_policy` vergleicht die Art selbst (`offered.ownership !=
requested.ownership`), ein Laufzeittest dafür fehlt.

### §2.2.3.9 OWNERSHIP — Changeable No

**Spec:** §2.2.3, S. 107 — OWNERSHIP: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.9.1 SHARED

**Spec:** §2.2.3.9.1, S. 117 — "The subscriber to the Topic will be able to
access modifications from all DataWriter objects … In any case there is no
“filtering” of modifications made based on the identity of the DataWriter that
causes the modification."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`:
SHARED lässt alles durch)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::shared_ownership_passes_all_samples`

**Status:** done

### §2.2.3.9.2 EXCLUSIVE — ein Eigentümer je Instanz, höchste Stärke

**Spec:** §2.2.3.9.2, S. 117 — "at any point in time a single DataWriter “owns”
each instance and is the only one whose modifications will be visible to the
DataReader objects. The owner is determined by selecting the DataWriter with the
highest value of the strength that is both “alive” as defined by the LIVELINESS
QoS policy and has not violated its DEADLINE contract with regards to the
data-instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::accept_under_exclusive_ownership`);
`crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`)

**Tests:**
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_filters_weaker_writer`,
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`,
`crates/dcps/src/instance_tracker.rs::tests::exclusive_higher_strength_wins`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — die höchste Stärke unter den Writern, die leben und ihre
DEADLINE für die Instanz einhalten.

### §2.2.3.9.2 EXCLUSIVE — Wechsel bei Liveliness, Stärke, DEADLINE

**Spec:** §2.2.3.9.2, S. 117 — "Ownership can therefore change as a result of
(a) a DataWriter in the system with a higher value of the strength that modifies
the instance, (b) a change in the strength value of the DataWriter that owns the
instance, (c) a change in the liveliness of the DataWriter that owns the
instance, and (d) a deadline with regards to the instance that is missed by the
DataWriter that owns the instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`accept_under_exclusive_ownership`, `update_owner_strength`,
`clear_owner_for_writer`); `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`)

**Tests:**
`crates/dcps/tests/ownership_failover.rs::notify_writer_liveliness_lost_clears_owner`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_lowers_its_strength_keeps_the_instance_until_a_stronger_one`,
`crates/dcps/src/instance_tracker.rs::tests::an_announced_strength_holds_the_owned_instances`

**Status:** `done` — (a) ein stärkerer Writer übernimmt; (b) die angekündigte
Stärke des Eigentümers gilt ab der Ankündigung; (c) Liveliness-Verlust oder
Wegfall der Zuordnung gibt das Eigentum frei; (d) ein Eigentümer, der seine
DEADLINE für die Instanz verpasst, verliert sie an einen anderen Writer.

### §2.2.3.9.2 EXCLUSIVE — gleiche Stärke, gleiche Wahl aller Reader

**Spec:** §2.2.3.9.2, S. 117 — "If this occurs the Service will pick one of the
DataWriter objects as the “owner.” It is not specified how the owner is
selected. However, it is required that the policy used to select the owner is
such that all DataReader objects will make the same choice".

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::accept_under_exclusive_ownership`: kleinere GUID);
`crates/qos/src/exclusive_ownership.rs`

**Tests:**
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_tie_break_by_lower_guid`,
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_higher_guid_at_tie_rejected`

**Status:** `done` — bei gleicher Stärke gewinnt der kleinere GUID, im
DataReader und im Resolver von `zerodds-qos` dieselbe Regel; Cyclone 11 wählt
ebenso (`inst_accepts_sample_by_writer_guid`).

### §2.2.3.9.2 EXCLUSIVE — je Instanz

**Spec:** §2.2.3.9.2, S. 118 — "Exclusive ownership is on an instance-by-instance
basis. That is, a subscriber can receive values written by a lower strength
DataWriter as long as they affect instances whose values have not been set by
the higher-strength DataWriter."

**Repo:** `crates/dcps/src/instance_tracker.rs` (Eigentümer je
`InstanceState`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::exclusive_first_writer_wins`,
`crates/dcps/tests/ownership_failover.rs::notify_participant_liveliness_lost_clears_all_writers_with_prefix`

**Status:** done

### §2.2.3.9.2 EXCLUSIVE — keine Rückmeldung an den Writer

**Spec:** §2.2.3.9.2, S. 117 — "It is also not a requirement that the DataWriter
objects are aware of whether they own a particular instance. There is no error
or notification given to a DataWriter that modifies an instance it does not
currently own."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Freistellung; der DataWriter von ZeroDDS
erfährt nichts über das Eigentum.

### §2.2.3.10 OWNERSHIP_STRENGTH — Feld value

**Spec:** §2.2.3, S. 108 — OWNERSHIP_STRENGTH für DataWriter: "An integer
“value”" — "[optional] Specifies the value of the “strength” used to arbitrate
among multiple DataWriter objects that attempt to modify the same instance".

**Repo:** `crates/qos/src/policies/ownership_strength.rs`
(`OwnershipStrengthQosPolicy`); `crates/dcps/src/qos.rs`
(`DataWriterQos::ownership_strength`)

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_negative`,
`crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_large`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — Default 0

**Spec:** §2.2.3, S. 108 — "The default value of the ownership_strength is
zero."

**Repo:** `crates/qos/src/policies/ownership_strength.rs`

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::default_is_zero`,
`crates/qos/src/defaults.rs::tests::ownership_strength_default_zero`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — RxO N/A

**Spec:** §2.2.3, S. 108 — OWNERSHIP_STRENGTH: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `DataWriterQos::ownership_strength`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — Changeable Yes

**Spec:** §2.2.3, S. 108 — OWNERSHIP_STRENGTH: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`:
`ownership_strength`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_matched_writer_announcing_new_strength_and_lifespan_is_taken_up`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — Arbitrierung durch den DataReader

**Spec:** §2.2.3.10, S. 118 — "It only applies to the situation case where
OWNERSHIP kind is set to EXCLUSIVE … The arbitration is performed by the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_filters_weaker_writer`

**Status:** done

### §2.2.3.11 LIVELINESS — Felder kind und lease_duration

**Spec:** §2.2.3, S. 108 — LIVELINESS für Topic, DataReader, DataWriter: "A
“kind”: AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC and a duration
“lease_duration”".

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessKind`,
`LivelinessQosPolicy`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::roundtrip`,
`crates/qos/src/policies/liveliness.rs::tests::try_from_u32_strict`

**Status:** done

### §2.2.3.11 LIVELINESS — Defaults

**Spec:** §2.2.3, S. 108 — "The default kind is AUTOMATIC and the default value
of the lease_duration is infinite."

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessQosPolicy::default`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::default_automatic_infinite`,
`crates/dcps/tests/liveliness_qos.rs::liveliness_default_infinite_keeps_counters_zero`

**Status:** done

### §2.2.3.11 LIVELINESS — RxO

**Spec:** §2.2.3.11, S. 118–119 — "1. the inequality “offered kind >= requested
kind” … AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC. 2. the inequality
“offered lease_duration <= requested lease_duration” evaluates to TRUE."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::liveliness_kind_and_lease_checked`,
`crates/qos/src/policies/qos_set.rs::tests::liveliness_lease_mismatch_reported`

**Status:** done — wie bei OWNERSHIP prüfen die Tests den Vergleich in
`zerodds-qos`; `first_incompatible_policy` rechnet ihn selbst. Dauer 0 und
INFINITE fallen wie bei DEADLINE zusammen.

### §2.2.3.11 LIVELINESS — Changeable No

**Spec:** §2.2.3, S. 108 — LIVELINESS: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.11 LIVELINESS — AUTOMATIC

**Spec:** §2.2.3, S. 108 — "The infrastructure will automatically signal
liveliness for the DataWriters at least as often as required by the
lease_duration."; §2.2.3.11, S. 118 — "as long as the local process where a
DomainParticipant is running and the link connecting it to remote participants
remains connected, the entities within the DomainParticipant will be considered
alive."

**Repo:** `crates/dcps/src/wlp.rs`; `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`,
`crates/dcps/src/liveliness_tracker.rs::tests::an_automatic_writer_is_alive_from_its_match_without_writing`

**Status:** done

### §2.2.3.11 LIVELINESS — MANUAL: Bestätigung innerhalb lease_duration

**Spec:** §2.2.3, S. 109 — "The user application takes responsibility to signal
liveliness to the Service … Liveliness must be asserted at least once every
lease_duration otherwise the Service will assume the corresponding Entity is no
longer “active/alive.”"; §2.2.3.11, S. 118 — "The action can be explicit by
calling the assert_liveliness operations, or implicit by writing some data."

**Repo:** `crates/dcps/src/liveliness_tracker.rs`; `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_writer_goes_not_alive_when_silent_and_back_when_it_writes`,
`crates/dcps/src/runtime.rs::tests::liveliness_lost_counts_a_silent_manual_writer_once_and_no_automatic_one`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`

**Status:** done

### §2.2.3.11 LIVELINESS — MANUAL_BY_PARTICIPANT

**Spec:** §2.2.3, S. 109 — "The Service will assume that as long as at least one
Entity within the DomainParticipant has asserted its liveliness the other
Entities in that same DomainParticipant are also alive."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`participant_asserted`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_liveliness`,
`DcpsRuntime::note_participant_liveliness`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_participant_message_asserts_only_what_its_kind_covers`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`

**Status:** done — die Wege zum entfernten Reader sind in `ddsi-rtps-2.5.md`
geführt (§8.4.13).

### §2.2.3.11 LIVELINESS — MANUAL_BY_TOPIC

**Spec:** §2.2.3, S. 109 — "The Service will only assume liveliness of the
DataWriter if the application has asserted liveliness of that DataWriter
itself."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_writer_liveliness_eid`)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`

**Status:** done

### §2.2.3.11 LIVELINESS — Erkennung innerhalb lease_duration

**Spec:** §2.2.3.11, S. 119 — "Changes in LIVELINESS must be detected by the
Service with a time-granularity greater or equal to the lease_duration. This
ensures that the value of the LivelinessChangedStatus is updated at least once
during each lease_duration and the related Listeners and WaitSets are notified
within a lease_duration from the time the LIVELINESS changed."

**Repo:** `crates/dcps/src/runtime.rs` (Tick der Lease-Prüfung);
`crates/dcps/src/liveliness_tracker.rs`

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — Feld minimum_separation

**Spec:** §2.2.3, S. 109 — TIME_BASED_FILTER für DataReader: "A duration
"minimum_separation"" — "The filter states that the DataReader does not want to
receive more than one value each minimum_separation, regardless of how fast the
changes occur."

**Repo:** `crates/qos/src/policies/time_based_filter.rs`
(`TimeBasedFilterQosPolicy`); `crates/dcps/src/qos.rs`
(`DataReaderQos::time_based_filter`); `crates/dcps/src/runtime.rs`
(`TimeBasedFilterClock`)

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::roundtrip`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::the_sender_clock_filters_on_the_source_timestamps`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::the_receiver_clock_filters_on_the_arrival`

**Status:** `done` — Die Spec legt die Uhr nicht fest, auf der
`minimum_separation` gemessen wird. Die Einstellung
`reader.time_based_filter_clock` wählt `sender` (Quellzeitstempel aus INFO_TS,
ohne ihn die Empfangszeit; Default) oder `receiver` (Empfangszeit). Gemessen
wurde vorher mit der Zeit des Lesens.

### §2.2.3.12 TIME_BASED_FILTER — Default 0

**Spec:** §2.2.3, S. 109 — "By default minimum_separation=0 indicating
DataReader is potentially interested in all values."

**Repo:** `crates/qos/src/policies/time_based_filter.rs`

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::default_is_zero`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_zero_passes_all_samples`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — RxO N/A

**Spec:** §2.2.3, S. 109 — TIME_BASED_FILTER: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `DataReaderQos::time_based_filter`)

**Tests:** `crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_filters_close_samples_per_instance`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — Changeable Yes

**Spec:** §2.2.3, S. 109 — TIME_BASED_FILTER: Changeable "Yes".

**Repo:** `crates/dcps/src/subscriber.rs` (`runtime_qos_update`:
`time_based_filter`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — je Instanz

**Spec:** §2.2.3.12, S. 119 — "The TIME_BASED_FILTER applies to each instance
separately, that is, the constraint is that the DataReader does not want to see
more than one sample of each instance per minimum_separation period."

**Repo:** `crates/dcps/src/instance_tracker.rs` (Filter je Instanz)

**Tests:** `crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_is_per_instance`,
`crates/dcps/src/instance_tracker.rs::tests::time_based_filter_per_instance_isolation`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — vor HISTORY und RELIABILITY

**Spec:** §2.2.3.12, S. 119 — "The HISTORY and RELIABILITY QoS affect the
behavior of the middleware with respect to the samples that have been determined
to be of interest to the DataReader, that is, they apply after the
TIME_BASED_FILTER has been applied."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`: Filter
vor der Aufnahme in den Cache)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::time_based_filter_too_close_drops`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — RELIABLE: letztes Sample im Ruhezustand

**Spec:** §2.2.3.12, S. 119 — "In the case where the reliability QoS kind is
RELIABLE then in steady-state, defined as the situation where the DataWriter does
not write new samples for a period “long” compared to the minimum_separation,
the system should guarantee delivery the last sample to the DataReader."

**Repo:** `crates/dcps/src/presentation.rs` (`TbfHold`);
`crates/dcps/src/subscriber.rs` (`DataReader::release_due_tbf`,
`DataReader::arm_tbf_hook`); `crates/dcps/src/runtime.rs`
(`UserReaderSlot::tbf_gate`, `release_due_tbf`)

**Tests:**
`crates/dcps/src/presentation.rs::tests::the_last_filtered_sample_of_an_instance_is_due_a_separation_after_it_arrived`,
`crates/dcps/src/presentation.rs::tests::a_sample_that_passes_the_filter_supersedes_the_held_one`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::a_reliable_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::a_best_effort_reader_gets_no_filtered_sample`,
`crates/zerodds-c-api/tests/time_based_filter.rs::a_reliable_c_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet`

**Status:** `done` — Ein RELIABLE-Reader hält je Instanz das letzte gefilterte
Sample und liefert es `minimum_separation` nach seiner Ankunft aus, wenn bis
dahin keines durchkam; ein Tick-Hook setzt dafür DATA_AVAILABLE. Typisierter
Reader und Runtime-Pfad (C-API, rmw).

### §2.2.3.12 TIME_BASED_FILTER — Konsistenz mit DEADLINE

**Spec:** §2.2.3.12, S. 119 — "For these two QoS policies to be consistent they
must verify that "period >= minimum_separation." An attempt to set these
policies in an inconsistent manner when an entity is created of via a set_qos
operation will cause the operation to fail."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::a_time_based_filter_longer_than_the_deadline_refuses_the_reader`

**Status:** `done` — wie unter §2.2.3.7.

### §2.2.3.13 PARTITION — Feld name

**Spec:** §2.2.3, S. 109 — PARTITION für Publisher, Subscriber: "A list of
strings “name”" — "A DataWriter within a Publisher only communicates with a
DataReader in a Subscriber if … the Publisher and Subscriber have a common
partition name string."

**Repo:** `crates/qos/src/policies/partition.rs` (`PartitionQosPolicy::names`);
`crates/dcps/src/qos.rs` (`PublisherQos::partition`, `SubscriberQos::partition`;
Überschreibung je Endpoint, `effective_partition`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::roundtrip_two_names`,
`crates/dcps/src/runtime.rs::tests::partition_from_publisher_reaches_the_writer_announcement`

**Status:** done

### §2.2.3.13 PARTITION — Default und leere Zeichenkette

**Spec:** §2.2.3, S. 109 — "The empty string ("") is considered a valid
partition … The default value for the PARTITION QoS is a zero-length sequence.
The zero-length sequence is treated as a special value equivalent to a sequence
containing a single element consisting of the empty string."

**Repo:** `crates/qos/src/policies/partition.rs` (`partitions_compatible`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::default_is_empty`,
`crates/qos/src/policies/partition.rs::tests::partitions_default_and_empty_string_are_equivalent`,
`crates/qos/src/policies/partition.rs::tests::partitions_named_does_not_match_default`

**Status:** done

### §2.2.3.13 PARTITION — RxO No, kein Unverträglichkeitsstatus

**Spec:** §2.2.3, S. 109 — PARTITION: RxO "No"; §2.2.3.13, S. 119 — "Failure to
match partitions is not considered an “incompatible” QoS and does not trigger any
listeners nor conditions."

**Repo:** `crates/dcps/src/runtime.rs` (Partitionsprüfung getrennt von
`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::partition_mismatch_blocks_match_without_any_incompatible_qos_event`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** done

### §2.2.3.13 PARTITION — Changeable Yes, neue und getrennte Zuordnungen

**Spec:** §2.2.3, S. 109 — PARTITION: Changeable "Yes"; §2.2.3.13, S. 119 — "A
change of this policy can potentially modify the “match” of existing DataReader
and DataWriter entities. It may establish new “matchs” that did not exist before,
or break existing matchs."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`),
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::update_user_group_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_moves_a_same_runtime_pair_apart_and_together`

**Status:** done

### §2.2.3.13 PARTITION — Platzhalter nach fnmatch

**Spec:** §2.2.3.13, S. 119 — "A partition name may contain wildcards. Sharing a
common partition means that one of the partition names matches … PARTITION names
can be regular expressions and include wildcards as defined by the POSIX fnmatch
API (1003.2-1992 section B.6). Either Publisher or Subscriber may include
regular expressions in partition names".

**Repo:** `crates/qos/src/policies/partition.rs` (`fnmatch`,
`partitions_compatible`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::fnmatch_star_matches_any`,
`crates/qos/src/policies/partition.rs::tests::fnmatch_char_class`,
`crates/qos/src/policies/partition.rs::tests::partitions_wildcard_matches_symmetrically`,
`crates/dcps/src/runtime.rs::tests::partitions_overlap_matches_canonical_fnmatch`

**Status:** done

### §2.2.3.13 PARTITION — zwei Platzhalter-Namen passen nie

**Spec:** §2.2.3.13, S. 119 — "but no two names that both contain wildcards will
ever be considered to match. This means that although regular expressions may be
used both at publisher as well as subscriber side, the service will not try to
match two regular expressions".

**Repo:** `crates/qos/src/policies/partition.rs` (`partition_names_match`,
`has_wildcard`)

**Tests:**
`crates/qos/src/policies/partition.rs::tests::two_wildcard_names_never_match`

**Status:** `done` — zwei Namen, die beide Platzhalter enthalten, passen nicht;
ein Platzhalter gegen einen festen Namen wie bisher.

### §2.2.3.13 PARTITION — Unterschied zu Domains

**Spec:** §2.2.3.13, S. 120 — "Partitions are different from creating Entity
objects in different domains in several ways … each unique data instance is
identified by the tuple (domainId, Topic, key)."

**Repo:** —

**Tests:** `crates/dcps/tests/partition_instances.rs::one_key_in_two_partitions_is_one_instance`

**Status:** done — ein Reader in den Partitionen A und B, der den Schlüssel 7 von einem Writer in A und von einem in B erhält, führt eine Instanz (ein `InstanceHandle`).

### §2.2.3.14 RELIABILITY — Felder kind und max_blocking_time

**Spec:** §2.2.3, S. 110 — RELIABILITY für Topic, DataReader, DataWriter: "A
“kind”: RELIABLE, BEST_EFFORT and a duration “max_blocking_time”".

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind`,
`ReliabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_roundtrip_with_custom_blocking`

**Status:** done

### §2.2.3.14 RELIABILITY — Defaults

**Spec:** §2.2.3, S. 110 — RELIABLE "This is the default value for
DataWriters"; BEST_EFFORT "This is the default value for DataReaders and Topics";
"The default max_blocking_time=100ms."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityQosPolicy::default`);
`crates/dcps/src/qos.rs` (`DataWriterQos::default`, `DataReaderQos::default`,
`TopicQos::default`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::default_is_best_effort_100ms`,
`crates/qos/src/review_tests.rs::writer_qos_default_is_reliable`,
`crates/qos/src/review_tests.rs::reader_qos_default_is_best_effort`

**Status:** done

### §2.2.3.14 RELIABILITY — RxO: offered kind >= requested kind

**Spec:** §2.2.3.14, S. 120 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … BEST_EFFORT < RELIABLE."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::all_four_reliability_combinations_follow_rtps_8_4_4`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.3.14 RELIABILITY — Changeable No

**Spec:** §2.2.3, S. 110 — RELIABILITY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`

**Status:** done

### §2.2.3.14 RELIABILITY — RELIABLE: Reparatur und Reihenfolge

**Spec:** §2.2.3, S. 110 — "Specifies the Service will attempt to deliver all
samples in its history. Missed samples may be retried."; §2.2.3.14, S. 120 —
"data-samples originating from a single DataWriter cannot be made available to
the DataReader if there are previous data-samples that have not been received
yet due to a communication error."

**Repo:** `crates/rtps/src/reliable_reader.rs`; `crates/rtps/src/reliable_writer.rs`

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`,
`crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`

**Status:** done — die Protokollseite ist in `ddsi-rtps-2.5.md` geführt.

### §2.2.3.14 RELIABILITY — BEST_EFFORT: keine Wiederholung, keine ältere Version

**Spec:** §2.2.3.14, S. 120 — "If the RELIABILITY kind is set to BEST_EFFORT,
the service will not re-transmit missing data-samples. However for data-samples
originating from any one DataWriter the service will ensure they are stored in
the DataReader history in the same order they originated in the DataWriter."

**Repo:** `crates/rtps/src/reliable_reader.rs` (BEST_EFFORT-Pfad)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/reliable_reader.rs::tests::duplicate_data_is_rejected`

**Status:** done

### §2.2.3.14 RELIABILITY — max_blocking_time und RESOURCE_LIMITS

**Spec:** §2.2.3, S. 110 — "The value of the max_blocking_time indicates the
maximum time the operation DataWriter::write is allowed to block if the
DataWriter does not have space to store the value written."; §2.2.3.14, S. 120.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`)

**Tests:** `crates/dcps/src/publisher.rs::tests::write_blocks_until_drain_when_reliable_max_samples_reached`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — Feld value

**Spec:** §2.2.3, S. 110 — TRANSPORT_PRIORITY für Topic, DataWriter: "An integer
“value”"; §2.2.3.15, S. 120 — "Any value within the range of a 32-bit signed
integer may be chosen; higher values indicate higher priority."

**Repo:** `crates/qos/src/policies/transport_priority.rs`
(`TransportPriorityQosPolicy`); `crates/dcps/src/runtime.rs`
(`send_on_best_interface` mit Priorität)

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::roundtrip`,
`crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — Default 0

**Spec:** §2.2.3, S. 110 — "The default value of the transport_priority is
zero."

**Repo:** `crates/qos/src/policies/transport_priority.rs`

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::default_is_zero`,
`crates/qos/src/defaults.rs::tests::transport_priority_default_zero`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — RxO N/A

**Spec:** §2.2.3, S. 110 — TRANSPORT_PRIORITY: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `TopicQos` und `DataWriterQos`)

**Tests:** `crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — Changeable Yes

**Spec:** §2.2.3, S. 110 — TRANSPORT_PRIORITY: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`:
`transport_priority`)

**Tests:** `crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — Hinweis an den Transport

**Spec:** §2.2.3.15, S. 120 — "This policy is considered a hint. The policy
depends on the ability of the underlying transports to set a priority on the
messages they send … any further interpretation of this policy is specific to a
particular transport and a particular implementation of the Service."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Freistellung der Auslegung; der Weg des
Werts zum Transport ist im Item „Feld value“ belegt.

### §2.2.3.16 LIFESPAN — Feld duration

**Spec:** §2.2.3, S. 110 — LIFESPAN für Topic, DataWriter: "A duration
“duration”" — "Specifies the maximum duration of validity of the data written by
the DataWriter".

**Repo:** `crates/qos/src/policies/lifespan.rs` (`LifespanQosPolicy`)

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::roundtrip`

**Status:** done

### §2.2.3.16 LIFESPAN — Default unendlich

**Spec:** §2.2.3, S. 110 — "The default value of the lifespan duration is
infinite."

**Repo:** `crates/qos/src/policies/lifespan.rs`

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::default_is_infinite`,
`crates/qos/src/defaults.rs::tests::lifespan_default_infinite`

**Status:** done

### §2.2.3.16 LIFESPAN — RxO N/A

**Spec:** §2.2.3, S. 110 — LIFESPAN: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `TopicQos` und `DataWriterQos`)

**Tests:** `crates/dcps/tests/lifespan_qos.rs::linux::lifespan_removes_expired_samples_from_the_reader_cache`

**Status:** done

### §2.2.3.16 LIFESPAN — Changeable Yes

**Spec:** §2.2.3, S. 110 — LIFESPAN: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`: `lifespan`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_matched_writer_announcing_new_strength_and_lifespan_is_taken_up`

**Status:** done

### §2.2.3.16 LIFESPAN — Ablaufzeit aus Quellzeit plus Dauer, Entfernen aus dem Reader-Cache

**Spec:** §2.2.3.16, S. 120–121 — "Once the sample expires, the data will be
removed from the DataReader caches … The ‘expiration time’ of each sample is
computed by adding the duration specified by the LIFESPAN QoS to the source
timestamp."

**Repo:** `crates/dcps/src/subscriber.rs` (`lifespan_expiry`,
`purge_expired_by_lifespan`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::lifespan_expiry_adds_the_writer_duration_to_the_source_timestamp`,
`crates/dcps/src/subscriber.rs::tests::purge_expired_by_lifespan_drops_only_samples_past_their_expiration`,
`crates/dcps/tests/lifespan_qos.rs::linux::lifespan_removes_expired_samples_from_the_reader_cache`

**Status:** done

### §2.2.3.16 LIFESPAN — Entfernen aus TRANSIENT- und PERSISTENT-Speichern

**Spec:** §2.2.3.16, S. 120 — "Once the sample expires, the data will be removed
from the DataReader caches as well as from the transient and persistent
information caches."

**Repo:** —

**Tests:** `crates/dcps/tests/durability_store.rs::an_expired_sample_leaves_the_store`,
`crates/dcps/src/durability_service.rs::tests::in_memory_replay_drops_the_expired_sample`,
`crates/dcps/src/durability_service.rs::tests::on_disk_replay_drops_the_expired_sample`

**Status:** `done` — das Sample trägt `expires_at` (Quellzeit plus LIFESPAN des Writers, auf der Platte als `<sequence>.exp`); `replay_for_topic_at` und `cleanup` entfernen abgelaufene Samples und Instanzen ohne Sample.

### §2.2.3.16 LIFESPAN — Empfangszeit statt Quellzeit

**Spec:** §2.2.3.16, S. 121 — "If this is not the case and the Service can
detect it, the DataReader is allowed to use the reception timestamp instead of
the source timestamp in its computation of the ‘expiration time.’"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; ZeroDDS rechnet mit der
Quellzeit.

### §2.2.3.17 DESTINATION_ORDER — Feld kind

**Spec:** §2.2.3, S. 110–111 — DESTINATION_ORDER für Topic, DataReader,
DataWriter: "A “kind”: BY_RECEPTION_TIMESTAMP, BY_SOURCE_TIMESTAMP".

**Repo:** `crates/qos/src/policies/destination_order.rs`
(`DestinationOrderKind`, `DestinationOrderQosPolicy`)

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::roundtrip`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — Default BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.3, S. 110 — "The default kind is BY_RECEPTION_TIMESTAMP."

**Repo:** `crates/qos/src/policies/destination_order.rs`

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::default_by_reception`,
`crates/qos/src/defaults.rs::tests::destination_order_default_by_reception`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — RxO

**Spec:** §2.2.3.17, S. 121 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … BY_RECEPTION_TIMESTAMP < BY_SOURCE_TIMESTAMP."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_offered_destination_order_incompatible_rejects_match`,
`crates/dcps/src/runtime.rs::tests::rxo_requested_destination_order_incompatible_rejects_match`,
`crates/qos/src/compatibility.rs::tests::destination_order_offered_ge_requested`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — Changeable No

**Spec:** §2.2.3, S. 110 — DESTINATION_ORDER: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.3.17, S. 121 — "The setting BY_RECEPTION_TIMESTAMP indicates that,
assuming the OWNERSHIP policy allows it, the latest received value for the
instance should be the one whose value is kept."

**Repo:** `crates/dcps/src/instance_tracker.rs`

**Tests:** `crates/dcps/tests/destination_order_qos.rs::by_reception_timestamp_always_delivers`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — BY_SOURCE_TIMESTAMP

**Spec:** §2.2.3.17, S. 121 — "The setting BY_SOURCE_TIMESTAMP indicates that,
assuming the OWNERSHIP policy allows it, a timestamp placed at the source should
be used. This is the only setting that, in the case of concurrent same-strength
DataWriter objects updating the same instance, ensures all subscribers will end
up with the same final value for the instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`; `crates/dcps/src/subscriber.rs`
(`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/destination_order_qos.rs::by_source_timestamp_drops_older_samples`,
`crates/dcps/tests/destination_order_qos.rs::destination_order_is_per_instance`,
`crates/dcps/tests/time_source_destination_order.rs::test_clock_drives_destination_order_deterministically`

**Status:** done

### §2.2.3.18 HISTORY — Felder kind und depth

**Spec:** §2.2.3, S. 111 — HISTORY für Topic, DataReader, DataWriter: "A “kind”:
KEEP_LAST, KEEP_ALL And an optional integer “depth”".

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryKind`,
`HistoryQosPolicy`)

**Tests:** `crates/qos/src/policies/history.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/history.rs::tests::encode_decode_keep_all`

**Status:** done

### §2.2.3.18 HISTORY — Default KEEP_LAST, depth 1

**Spec:** §2.2.3, S. 111 — "KEEP_LAST is the default kind. The default value of
depth is 1."

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryQosPolicy::default`)

**Tests:** `crates/qos/src/policies/history.rs::tests::default_is_keep_last_1`,
`crates/qos/src/defaults.rs::tests::history_default_keep_last_1`

**Status:** done

### §2.2.3.18 HISTORY — RxO No

**Spec:** §2.2.3, S. 111 — HISTORY: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` ohne HISTORY)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** done

### §2.2.3.18 HISTORY — Changeable No

**Spec:** §2.2.3, S. 111 — HISTORY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`

**Status:** done

### §2.2.3.18 HISTORY — KEEP_LAST

**Spec:** §2.2.3, S. 111; §2.2.3.18, S. 121 — "On the publishing side, the
Service will only attempt to keep the most recent “depth” samples of each
instance of data … On the subscribing side, the DataReader will only attempt to
keep the most recent “depth” samples received for each instance … until the
application “takes” them".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::evict_oldest_of_instance`);
`crates/dcps/src/publisher.rs` (`create_datawriter`: Tiefe an die Laufzeit)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_last_1_keeps_only_the_newest_per_instance`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_is_independent_per_instance`,
`crates/dcps/src/publisher.rs::tests::create_datawriter_wires_history_keep_last_depth`

**Status:** done

### §2.2.3.18 HISTORY — KEEP_ALL

**Spec:** §2.2.3, S. 112; §2.2.3.18, S. 121 — "the Service will attempt to keep
all samples … If the limit is reached, then the behavior of the Service will
depend on the RELIABILITY QoS. If the reliability kind is BEST_EFFORT, then the
old values will be discarded. If reliability is RELIABLE, then the Service will
block the DataWriter until it can deliver the necessary old values to all
subscribers."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/subscriber.rs` (Aufnahme unter KEEP_ALL)

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`,
`crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** `done`

### §2.2.3.18 HISTORY — Konsistenz depth <= max_samples_per_instance

**Spec:** §2.2.3, S. 111; §2.2.3.18, S. 121 — "The setting of HISTORY depth must
be consistent with the RESOURCE_LIMITS max_samples_per_instance. For these two
QoS to be consistent, they must verify that depth <= max_samples_per_instance."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — eine KEEP_LAST-Tiefe über `max_samples_per_instance` und
eine Tiefe unter 1 werden abgelehnt.

### §2.2.3.19 RESOURCE_LIMITS — Felder

**Spec:** §2.2.3, S. 112 — RESOURCE_LIMITS für Topic, DataReader, DataWriter:
"Three integers: max_samples, max_instances, max_samples_per_instance".

**Repo:** `crates/qos/src/policies/resource_limits.rs`
(`ResourceLimitsQosPolicy`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/resource_limits.rs::tests::encode_decode_negative_one`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — Defaults LENGTH_UNLIMITED

**Spec:** §2.2.3, S. 112 — max_samples, max_instances,
max_samples_per_instance: "By default, LENGTH_UNLIMITED."

**Repo:** `crates/qos/src/policies/resource_limits.rs` (`LENGTH_UNLIMITED`,
`ResourceLimitsQosPolicy::default`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::default_is_unlimited`,
`crates/dcps/src/qos.rs::data_representation_tests::resource_limits_default_to_unlimited`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — RxO No

**Spec:** §2.2.3, S. 112 — RESOURCE_LIMITS: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` ohne RESOURCE_LIMITS)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_limited_writer_with_an_acknowledging_reader_writes_everything`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — Changeable No

**Spec:** §2.2.3, S. 112 — RESOURCE_LIMITS: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.19 RESOURCE_LIMITS — max_samples, max_instances, max_samples_per_instance

**Spec:** §2.2.3, S. 112 — "max_samples Specifies the maximum number of
data-samples the DataWriter (or DataReader) can manage across all the instances
… max_instances Represents the maximum number of instances DataWriter (or
DataReader) can manage … max_samples_per_instance Represents the maximum number
of samples of any one instance".

**Repo:** `crates/dcps/src/subscriber.rs` (Aufnahmeprüfung);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_writer_resource_limits`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_writer_refuses_an_instance_beyond_its_own_max_instances`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — BEST_EFFORT verwirft, RELIABLE blockiert

**Spec:** §2.2.3.19, S. 122 — "If reliability is BEST_EFFORT then the Service is
allowed to drop samples. If the reliability is RELIABLE, the Service will block
the DataWriter or discard the sample at the DataReader in order not to lose
existing samples."

**Repo:** `crates/dcps/src/subscriber.rs`; `crates/dcps/src/publisher.rs`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_discards_the_oldest_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_reliable_sample_refused_for_room_arrives_once_there_is_room`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** `done`

### §2.2.3.19 RESOURCE_LIMITS — Konsistenz

**Spec:** §2.2.3, S. 112; §2.2.3.19, S. 122 — "max_samples >=
max_samples_per_instance … depth <= max_samples_per_instance … An attempt to set
this policy to inconsistent values when an entity is created of via a set_qos
operation will cause the operation to fail."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), gerufen von
`create_topic`, `create_datawriter`, `create_datareader` und den `set_qos` von
Topic, DataWriter und DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`

**Status:** `done` — `max_samples` unter `max_samples_per_instance` wird beim
Erzeugen und in `set_qos` abgelehnt; die C-API prüft `is_consistent` zusätzlich
beim Umsetzen der C-QoS.

### §2.2.3.20 ENTITY_FACTORY — Feld autoenable_created_entities

**Spec:** §2.2.3, S. 112 — ENTITY_FACTORY für DomainParticipantFactory,
DomainParticipant, Publisher, Subscriber: "A boolean:
“autoenable_created_entities”" — "If autoenable_created_entities==TRUE the
factory will automatically enable each created Entity otherwise it will not."

**Repo:** `crates/qos/src/policies/entity_factory.rs`
(`EntityFactoryQosPolicy`); `crates/dcps/src/factory.rs`
(`DomainParticipantFactoryQos`); `crates/dcps/src/qos.rs`

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done

### §2.2.3.20 ENTITY_FACTORY — Default TRUE

**Spec:** §2.2.3, S. 112 — "By default, TRUE."

**Repo:** `crates/qos/src/policies/entity_factory.rs`

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::default_is_true`,
`crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`

**Status:** done

### §2.2.3.20 ENTITY_FACTORY — RxO No

**Spec:** §2.2.3, S. 112 — ENTITY_FACTORY: RxO "No".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Policy betrifft keine Zuordnung zwischen
Writer und Reader; sie wird nicht angekündigt.

### §2.2.3.20 ENTITY_FACTORY — Changeable Yes, wirkt auf später Erzeugte

**Spec:** §2.2.3, S. 112 — ENTITY_FACTORY: Changeable "Yes"; §2.2.3.20, S. 122
— "This policy is mutable. A change in the policy affects only the entities
created after the change; not the previously created entities."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_qos`:
`set_autoenable_children`), `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`

**Status:** done — der Wert wirkt beim Erzeugen und bei der Kaskade von
`enable`; schon erzeugte Entities werden durch die Änderung nicht
freigegeben.

### §2.2.3.20 ENTITY_FACTORY — create_* ruft enable

**Spec:** §2.2.3.20, S. 122 — "The setting of autoenable_created_entities to TRUE
indicates that the factory create_<entity> operation will automatically invoke
the enable operation each time a new Entity is created. Therefore, the Entity
returned by create_<entity> will already be enabled."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`),
`crates/dcps/src/participant.rs` (`autoenables_children`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — Feld autodispose_unregistered_instances

**Spec:** §2.2.3, S. 113 — WRITER_DATA_LIFECYCLE für DataWriter: "A boolean:
“autodispose_unregistered_instances”" — "Controls whether a DataWriter will
automatically dispose instances each time they are unregistered."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`WriterDataLifecycleQosPolicy`); `crates/dcps/src/qos.rs`
(`DataWriterQos::writer_data_lifecycle`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_roundtrip`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — Default TRUE

**Spec:** §2.2.3, S. 113 — "By default, TRUE."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_default_is_autodispose_true`,
`crates/qos/src/defaults.rs::tests::writer_data_lifecycle_default_autodispose`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — RxO N/A

**Spec:** §2.2.3, S. 113 — WRITER_DATA_LIFECYCLE: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `DataWriterQos`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — Changeable Yes

**Spec:** §2.2.3, S. 113 — WRITER_DATA_LIFECYCLE: Changeable "Yes".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.21 WRITER_DATA_LIFECYCLE — TRUE und FALSE

**Spec:** §2.2.3.21, S. 122–123 — "The setting ‘autodispose_unregistered_instances
= TRUE’ causes the DataWriter to dispose the instance each time it is
unregistered … The setting ‘autodispose_unregistered_instances = FALSE’ will not
cause this automatic disposition upon unregistering."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::unregister_instance_w_timestamp`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`,
`crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_false_unregister_only_decrements_writer_count`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — Löschen des DataWriters

**Spec:** §2.2.3.21, S. 123 — "the setting of the
autodispose_unregistered_instances flag will determine whether instances are
ultimately disposed when the DataWriter is deleted either directly by means of
the Publisher::delete_datawriter operation or indirectly as a consequence of
calling delete_contained_entities".

**Repo:** `crates/dcps/src/publisher.rs` (`WriterInner::unregister_all_instances`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_disposes_and_unregisters_its_instances`

**Status:** done — `Test_FinalInstanceState_2` der OMG-Suite ist mit diesem
Verhalten spec-konform (`internal/release/rc7-omg-findings.md`).

### §2.2.3.22 READER_DATA_LIFECYCLE — Felder

**Spec:** §2.2.3, S. 113 — READER_DATA_LIFECYCLE für DataReader: "Two durations
“autopurge_nowriter_samples_delay” and “autopurge_disposed_samples_delay”".

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`ReaderDataLifecycleQosPolicy`); `crates/dcps/src/qos.rs`
(`DataReaderQos::reader_data_lifecycle`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_roundtrip`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — Defaults unendlich

**Spec:** §2.2.3, S. 113 — beide Dauern "By default, infinite."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_default_is_infinite`,
`crates/dcps/tests/reader_data_lifecycle_qos.rs::default_infinite_delay_keeps_disposed_instance`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — RxO N/A

**Spec:** §2.2.3, S. 113 — READER_DATA_LIFECYCLE: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (nur `DataReaderQos`)

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — Changeable Yes

**Spec:** §2.2.3, S. 113 — READER_DATA_LIFECYCLE: Changeable "Yes".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — der Test deckt es ab.

### §2.2.3.22 READER_DATA_LIFECYCLE — autopurge_nowriter_samples_delay

**Spec:** §2.2.3.22, S. 123 — "The autopurge_nowriter_samples_delay defines the
maximum duration for which the DataReader will maintain information regarding an
instance once its instance_state becomes NOT_ALIVE_NO_WRITERS. After this time
elapses, the DataReader will purge all internal information regarding the
instance, any untaken samples will also be lost."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`);
`crates/dcps/src/instance_tracker.rs`

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_nowriter_delay_purges_after_window`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_no_writers_after_delay`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — autopurge_disposed_samples_delay

**Spec:** §2.2.3.22, S. 123 — "The autopurge_disposed_samples_delay defines the
maximum duration for which the DataReader will maintain samples for an instance
once its instance_state becomes NOT_ALIVE_DISPOSED. After this time elapses, the
DataReader will purge all samples for the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`)

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_disposed_after_delay`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_disposed_before_delay_keeps_instance`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — Zustand nach dem letzten take

**Spec:** §2.2.3.22, S. 123 — "The DataReader also maintains information
regarding the identity, view_state and instance_state of data-instances even
after all samples have been ‘taken.’ This is needed to properly compute the
states when future samples arrive."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker` bleibt nach
`drain_samples` bestehen)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`,
`crates/dcps/src/instance_tracker.rs::tests::drain_samples_decrements_count`

**Status:** done

### §2.2.3.23 Registrierung, LIVELINESS und OWNERSHIP — Anwendungsfälle

**Spec:** §2.2.3.23, S. 123 — die Registrierung dient der Eigentumsauflösung
in redundanten Systemen und der Erkennung verlorener Verbindungen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung; die Anforderungen stehen in den
folgenden Items und unter §2.2.3.9.

### §2.2.3.23.1 Eigentumsauflösung in redundanten Systemen

**Spec:** §2.2.3.23.1, S. 123–124 — Ursachen [crash], [connectivity loss],
[application fault]; "Switching ownership from a higher strength DataWriter to
one of a lower strength DataWriter requires that the DataReader can make a
determination that the stronger DataWriter is “no longer writing the
instance.”"

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`, `passes_exclusive_ownership`);
`crates/dcps/src/instance_tracker.rs` (`accept_under_exclusive_ownership`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — das Eigentum wird bei Liveliness-Verlust, Wegfall der
Zuordnung, `unregister_instance` und verpasster DEADLINE freigegeben.

### §2.2.3.23.1 unregister_instance des Eigentümers übergibt das Eigentum

**Spec:** §2.2.3.23.3, S. 125 — "For a Topic with EXCLUSIVE OWNERSHIP if the
current owner of an instance unregisters it, then it will relinquish ownership
of the instance and thus the readers may see the value updated by another
writer"; §2.2.2.4.2.7, S. 64.

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::release_owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`

**Status:** `done` — meldet der Eigentümer die Instanz ab, übernimmt sie der
schwächere Writer mit seinem nächsten Sample.

### §2.2.3.23.1.1 Periodisch geschriebene Daten — DEADLINE

**Spec:** §2.2.3.23.1.1, S. 124 — "If the deadline is missed, the DataReader
considers the DataWriter “not alive” and automatically gives ownership to the
next highest-strength DataWriter that is alive."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`accept_under_exclusive_ownership` mit `deadline_nanos`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — ein Eigentümer, dessen letztes Sample der Instanz älter als
die DEADLINE ist, gilt für die Wahl als nicht lebendig; das nächste Sample eines
anderen Writers übernimmt.

### §2.2.3.23.1.2 Nicht periodisch geschriebene Daten — LIVELINESS

**Spec:** §2.2.3.23.1.2, S. 124 — "Ownership is maintained while the DataWriter
is “alive” and for the DataWriter to be alive it must fulfill its “LIVELINESS”
QoS contract … Hence the need for the writer to “register” and “unregister”
instances … unregistration will also require a message to be sent to the
readers."

**Repo:** `crates/dcps/src/publisher.rs`
(`DataWriter::unregister_instance_w_timestamp`: Abmeldung auf der Leitung);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:**
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`,
`crates/dcps/tests/ownership_failover.rs::notify_writer_liveliness_lost_clears_owner`

**Status:** `done` — die Abmeldung geht als Nachricht an die Reader; ein
Liveliness-Verlust des Eigentümers gibt das Eigentum ab.

### §2.2.3.23.2 Erkennung verlorener Verbindungen

**Spec:** §2.2.3.23.2, S. 125 — "For the case where data is being written
periodically, the DEADLINE QoS and the on_deadline_missed listener provides the
notification. The case where data is not periodically updated requires the use
of the LIVELINESS in combination with register/unregister instance to detect
whether the “connectivity” has been lost, and the notification is provided by
means of the “NO_WRITERS” view state."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, Liveliness-Prüfung);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done` — DEADLINE meldet über REQUESTED_DEADLINE_MISSED, der Verlust
der Liveliness über LIVELINESS_CHANGED und NOT_ALIVE_NO_WRITERS, das Löschen
eines Writers über NOT_ALIVE_NO_WRITERS.

### §2.2.3.23.3 dispose gegenüber unregister_instance

**Spec:** §2.2.3.23.3, S. 125 — "The dispose operation indicates that the
data-instance no longer exists … whereas the unregister_instance operation
indicates that the writer is no longer taking responsibility for updating the
value of the instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose`,
`DataWriter::unregister_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_unregister_drops_to_no_writers_when_count_zero`

**Status:** done

### §2.2.3.23.3 EXCLUSIVE: dispose des Eigentümers

**Spec:** §2.2.3.23.3, S. 125 — "For a Topic with EXCLUSIVE OWNERSHIP if the
current owner of an instance disposes it, the readers accessing the instance
will see the instance_state as being “DISPOSED” and not see the values being
written by the weaker writer (even after the stronger one has disposed the
instance)."

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`: Eigentümerprüfung
für `dispose`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** `done` — der Eigentümer disposed, ein schwächerer Writer kann weder
disposen noch danach schreiben.

### §2.2.4 Zwei Mechanismen

**Spec:** §2.2.4, S. 125 — "Listeners and conditions (in conjunction with
wait-sets) are two alternative mechanisms that allow the application to be made
aware of changes in the DCPS communication status."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Überblick; die Anforderungen stehen in den
folgenden Unterabschnitten.

### §2.2.4.1 Status je Entity

**Spec:** §2.2.4.1, S. 126 — Tabelle: Topic INCONSISTENT_TOPIC; Subscriber
DATA_ON_READERS; DataReader SAMPLE_REJECTED, LIVELINESS_CHANGED,
REQUESTED_DEADLINE_MISSED, REQUESTED_INCOMPATIBLE_QOS, DATA_AVAILABLE,
SAMPLE_LOST, SUBSCRIPTION_MATCHED; DataWriter LIVELINESS_LOST,
OFFERED_DEADLINE_MISSED, OFFERED_INCOMPATIBLE_QOS, PUBLICATION_MATCHED.

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`);
`crates/dcps/src/status.rs`; `get_*_status` je Entity in
`crates/dcps/src/topic.rs`, `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done — die Zuordnung zur Entity ist je Status unten und bei den
`get_*_status`-Operationen (§2.2.2.3 bis §2.2.2.5) belegt.

### §2.2.4.1 Lese-Status und einfache Status

**Spec:** §2.2.4.1, S. 126 — "Read communication statuses: i.e., those that are
related to arrival of data, namely DATA_ON_READERS and DATA_AVAILABLE. Plain
communication statuses: i.e., all the others. … This ‘grouping’ has to be
communicated to the application. How this is done is discussed in each of the
two following sub clauses."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einteilung; die Regeln stehen unter §2.2.4.2
und §2.2.4.3.

### §2.2.4.1 Eine Struktur je einfachem Status

**Spec:** §2.2.4.1, S. 127 — "For each plain communication status, there is a
corresponding structure to hold the status value. These values contain the
information related to the change of status, as well as information related to
the statuses themselves (e.g., contains cumulative counts)." Figure 2.13.

**Repo:** `crates/dcps/src/status.rs` (`InconsistentTopicStatus`,
`SampleLostStatus`, `SampleRejectedStatus`, `LivelinessLostStatus`,
`LivelinessChangedStatus`, `OfferedDeadlineMissedStatus`,
`RequestedDeadlineMissedStatus`, `OfferedIncompatibleQosStatus`,
`RequestedIncompatibleQosStatus`, `PublicationMatchedStatus`,
`SubscriptionMatchedStatus`, `QosPolicyCount`)

**Tests:** `crates/dcps/src/status.rs::tests::inconsistent_topic_default_is_zero`,
`crates/dcps/src/status.rs::tests::publication_matched_with_handle_roundtrip`,
`crates/dcps/src/status.rs::tests::offered_incompatible_qos_clone_roundtrip`

**Status:** done — die Felder haben die Namen aus Figure 2.13; `long` ist
`i32`, `QosPolicyId_t` ist `u32`. DATA_AVAILABLE und DATA_ON_READERS haben
leere Strukturen (`DataAvailableStatus`, `DataOnReadersStatus`).

### §2.2.4.1 InconsistentTopicStatus.total_count

**Spec:** §2.2.4.1, S. 128 — "Total cumulative count of the Topics discovered
whose name matches the Topic to which this status is attached and whose type is
inconsistent with the Topic."

**Repo:** `crates/dcps/src/topic.rs` (`inconsistent_topic_total`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::record_inconsistent_topic`,
`inconsistent_topics_counted`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::xtypes_step1_refuses_a_differently_named_non_assignable_type`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/src/runtime.rs::tests::an_inconsistent_topic_counts_once_per_remote_participant`

**Status:** `done` — gezählt wird ein lokales `create_topic` mit gleichem Namen
und abweichender QoS und jedes entdeckte entfernte Topic mit unverträglichem Typ
einmal.

### §2.2.4.1 InconsistentTopicStatus.total_count — je entdecktem Topic

**Spec:** §2.2.4.1, S. 128 — "Total cumulative count of the Topics discovered
whose name matches the Topic".

**Repo:** `crates/dcps/src/runtime.rs`
(`DcpsRuntime::record_inconsistent_topic`, `forget_inconsistent_topics_of`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::an_inconsistent_topic_counts_once_per_remote_participant`

**Status:** `done` — ein entferntes Topic zählt je entferntem Participant
einmal, bis der Participant geht, gleich wie viele Endpoints es zeigt; das
lokale `create_topic` weckt den Listener.

### §2.2.4.1 InconsistentTopicStatus.total_count_change

**Spec:** §2.2.4.1, S. 128 — "The incremental number of inconsistent topics
discovered since the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`

**Status:** done — der Test deckt es ab.

### §2.2.4.1 SampleLostStatus.total_count

**Spec:** §2.2.4.1, S. 126, 128 — SAMPLE_LOST "A sample has been lost (never
received)"; total_count "Total cumulative count of all samples lost across of
instances of data published under the Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`note_lost`,
`DcpsRuntime::record_sample_lost`); `crates/dcps/src/subscriber.rs`
(`DataReader::note_sample_lost`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::samples_a_best_effort_reader_steps_over_are_sample_lost`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`

**Status:** `done` — gezählt werden die Lücken, die ein BEST_EFFORT-Reader
überspringt, und was ein gegangener Writer vor gehaltenen Samples fehlen ließ;
verdrängte, empfangene Samples zählen nicht.

### §2.2.4.1 SampleLostStatus.total_count — nur nie empfangene Samples

**Spec:** §2.2.4.1, S. 126 — "SAMPLE_LOST A sample has been lost (never
received)."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`)

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::on_sample_rejected_listener_fires_for_a_best_effort_reader`

**Status:** `done` — ein unter BEST_EFFORT verdrängtes Sample ändert keinen
Status (§2.2.3.18: „the old values will be discarded“); das für `max_instances`
abgewiesene ist SAMPLE_REJECTED mit `REJECTED_BY_INSTANCES_LIMIT`, auch im
C-API.

### §2.2.4.1 SampleLostStatus.total_count_change

**Spec:** §2.2.4.1, S. 128 — "The incremental number of samples lost since the
last time the listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger`, `StatusRecord for
SampleLostStatus`)

**Tests:** `crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`,
`crates/dcps/src/status.rs::tests::a_change_after_the_look_survives_the_acknowledgement`

**Status:** done

### §2.2.4.1 SampleRejectedStatus.total_count

**Spec:** §2.2.4.1, S. 128 — "Total cumulative count of samples rejected by the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`,
`DataReader::note_sample_rejected`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::record_sample_rejected`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_reliable_sample_refused_for_room_arrives_once_there_is_room`

**Status:** done — ein zurückgehaltenes RELIABLE-Sample zählt einmal, auch wenn
es mehrmals geprüft wird.

### §2.2.4.1 SampleRejectedStatus.total_count_change

**Spec:** §2.2.4.1, S. 128 — "The incremental number of samples rejected since
the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::get_sample_rejected_status_reports_the_change_once`

**Status:** done — der Test deckt es ab.

### §2.2.4.1 SampleRejectedStatus.last_reason

**Spec:** §2.2.4.1, S. 128 — "Reason for rejecting the last sample rejected. If
no samples have been rejected, the reason is the special value NOT_REJECTED."

**Repo:** `crates/dcps/src/status.rs` (`SampleRejectedStatusKind`)

**Tests:** `crates/dcps/src/status.rs::tests::sample_rejected_status_default_kind_is_not_rejected`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`

**Status:** done

### §2.2.4.1 SampleRejectedStatus.last_instance_handle

**Spec:** §2.2.4.1, S. 128 — "Handle to the instance being updated by the last
sample that was rejected."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`

**Status:** done — bei `RejectedByInstancesLimit` ist der Handle NIL, weil für
das abgelehnte Sample keine Instanz angelegt wurde; bei einem Topic ohne
Schlüssel ebenso.

### §2.2.4.1 LivelinessChangedStatus.alive_count

**Spec:** §2.2.4.1, S. 128 — "The total number of currently active DataWriters
that write the Topic read by the DataReader. This count increases when a newly
matched DataWriter asserts its liveliness for the first time or when a
DataWriter previously considered to be not alive reasserts its liveliness. The
count decreases when a DataWriter considered alive fails to assert its liveliness
and becomes not alive, whether because it was deleted normally or for some other
reason."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::two_automatic_writers_are_two_alive_writers`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_manual_writer_counts_from_its_first_assertion`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/src/liveliness_tracker.rs::tests::unmatching_takes_a_writer_out_of_the_number_it_was_in`

**Status:** done — ein AUTOMATIC-Writer zählt ab seiner Zuordnung als lebendig.

### §2.2.4.1 LivelinessChangedStatus.not_alive_count

**Spec:** §2.2.4.1, S. 128 — "The total count of currently DataWriters that
write the Topic read by the DataReader that are no longer asserting their
liveliness. This count increases when a DataWriter considered alive fails to
assert its liveliness and becomes not alive for some reason other than the normal
deletion of that DataWriter. It decreases when a previously not alive DataWriter
either reasserts its liveliness or is deleted normally."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::expire`,
`LivelinessTracker::unmatched`, `lost_with_participant`)

**Tests:**
`crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/src/liveliness_tracker.rs::tests::unmatching_takes_a_writer_out_of_the_number_it_was_in`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts`

**Status:** `done` — Ablauf des Lease, Verlust mit dem Teilnehmer, erneute
Zusicherung und Löschung.

### §2.2.4.1 LivelinessChangedStatus.not_alive_count — Verlust ohne Löschung

**Spec:** §2.2.4.1, S. 128 — "This count increases when a DataWriter considered
alive fails to assert its liveliness and becomes not alive for some reason other
than the normal deletion of that DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`Departure::Lost`);
`crates/dcps/src/liveliness_tracker.rs` (`lost_with_participant`)

**Tests:**
`crates/dcps/src/liveliness_tracker.rs::tests::a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts`,
`crates/dcps/tests/liveliness_qos.rs::linux::a_writer_lost_with_its_participant_counts_as_not_alive`

**Status:** `done` — läuft das SPDP-Lease eines Teilnehmers ab, gehen seine
Writer von `alive_count` nach `not_alive_count` und bleiben dort, bis sie unter
ihrer GUID wieder zugeordnet sind und sich zusichern; SPDP- und SEDP-Dispose
bleiben Löschung. Cyclone 11 (`ddsi_delete_proxy_writer`) und OpenDDS
(`DataReaderImpl::writer_removed`) zählen beides als Löschung.

### §2.2.4.1 LivelinessChangedStatus.alive_count_change und not_alive_count_change

**Spec:** §2.2.4.1, S. 128 — "The change in the alive_count since the last time
the listener was called or the status was read … The change in the
not_alive_count since the last time the listener was called or the status was
read."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::take_status`,
`LivelinessTracker::acknowledge`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_listener_acknowledges_what_it_was_handed_and_no_more`,
`crates/dcps/src/status.rs::tests::liveliness_changed_acknowledges_both_changes`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.4.1 LivelinessChangedStatus.last_publication_handle

**Spec:** §2.2.4.1, S. 128 — "Handle to the last DataWriter whose change in
liveliness caused this status to change."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::status`:
`InstanceHandle::from_guid`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.4.1 RequestedDeadlineMissedStatus.total_count

**Spec:** §2.2.4.1, S. 129 — "Total cumulative number of missed deadlines
detected for any instance read by the DataReader. Missed deadlines accumulate;
that is, each deadline period the total_count will be incremented by one for
each instance for which data was not received."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, `missed_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done — gezählt wird je Instanz, die schon ein Sample erhielt; vor
dem ersten Sample einer Instanz läuft keine Frist.

### §2.2.4.1 RequestedDeadlineMissedStatus.total_count_change

**Spec:** §2.2.4.1, S. 129 — "The incremental number of deadlines detected since
the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::requested_deadline_missed_total_count_change_is_read_and_reset`

**Status:** done — der Test liest `total_count` und `total_count_change` vor und nach dem Lesen und nach einer weiteren Änderung.

### §2.2.4.1 RequestedDeadlineMissedStatus.last_instance_handle

**Spec:** §2.2.4.1, S. 129 — "Handle to the last instance in the DataReader for
which a deadline was detected."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::with_missed_instance`);
`crates/dcps/src/runtime.rs` (`missed_deadline_instance`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done

### §2.2.4.1 RequestedIncompatibleQosStatus.total_count

**Spec:** §2.2.4.1, S. 129 — "Total cumulative number of times the concerned
DataReader discovered a DataWriter for the same Topic with an offered QoS that
was incompatible with that requested by the DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_requested_incompatible`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`,
`crates/dcps/src/runtime.rs::tests::an_incompatible_remote_endpoint_is_counted_once`

**Status:** done — ein Writer zählt je Policy einmal, bis das Paar zugeordnet
wird oder der Writer geht.

### §2.2.4.1 RequestedIncompatibleQosStatus.total_count_change

**Spec:** §2.2.4.1, S. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for
RequestedIncompatibleQosStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 RequestedIncompatibleQosStatus.last_policy_id

**Spec:** §2.2.4.1, S. 129 — "The QosPolicyId_t of one of the policies that was
found to be incompatible the last time an incompatibility was detected."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`DcpsRuntime::note_requested_incompatible`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done — es ist die erste unverträgliche Policy in der Reihenfolge
von `first_incompatible_policy`.

### §2.2.4.1 RequestedIncompatibleQosStatus.policies

**Spec:** §2.2.4.1, S. 129 — "A list containing for each policy the total number
of times that the concerned DataReader discovered a DataWriter for the same Topic
with an offered QoS that is incompatible with that requested by the DataReader."

**Repo:** `crates/dcps/src/status.rs` (`bump_policy_count`)

**Tests:** `crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_coherent_access_unoffered_rejects_on_both_sides`

**Status:** done — je abgelehntem Writer wird die eine Policy gezählt, die
`first_incompatible_policy` nennt; weitere unverträgliche Policies desselben
Paares zählen nicht.

### §2.2.4.1 LivelinessLostStatus.total_count

**Spec:** §2.2.4.1, S. 129 — "Total cumulative number of times that a
previously-alive DataWriter became not alive due to a failure to actively signal
its liveliness within its offered liveliness period. This count does not change
when an already not alive DataWriter simply remains not alive for another
liveliness period."

**Repo:** `crates/dcps/src/runtime.rs` (`check_writer_liveliness`,
`liveliness_lost`)

**Tests:** `crates/dcps/src/runtime.rs::tests::liveliness_lost_counts_a_silent_manual_writer_once_and_no_automatic_one`

**Status:** done

### §2.2.4.1 LivelinessLostStatus.total_count_change

**Spec:** §2.2.4.1, S. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::liveliness_lost_total_count_change_is_read_and_reset`

**Status:** done — wie oben.

### §2.2.4.1 OfferedDeadlineMissedStatus.total_count

**Spec:** §2.2.4.1, S. 129 — "Total cumulative number of offered deadline
periods elapsed during which a DataWriter failed to provide data. Missed
deadlines accumulate; that is, each deadline period the total_count will be
incremented by one."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, `missed_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_increments_when_writer_misses_period`

**Status:** done — gezählt wird je Periode und Instanz, wie §2.2.3.7 den Vertrag
je Instanz fasst; der Unterschied zum Wortlaut für den Reader ist in der
Befunddatei geführt.

### §2.2.4.1 OfferedDeadlineMissedStatus.total_count_change

**Spec:** §2.2.4.1, S. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::offered_deadline_missed_total_count_change_is_read_and_reset`

**Status:** done — wie oben.

### §2.2.4.1 OfferedDeadlineMissedStatus.last_instance_handle

**Spec:** §2.2.4.1, S. 129 — "Handle to the last instance in the DataWriter for
which an offered deadline was missed."

**Repo:** `crates/dcps/src/publisher.rs` (`WriterStatus`: `last_instance_handle`
aus `missed_deadline_instance`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.total_count

**Spec:** §2.2.4.1, S. 129 — "Total cumulative number of times the concerned
DataWriter discovered a DataReader for the same Topic with a requested QoS that
is incompatible with that offered by the DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_offered_incompatible`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`,
`crates/dcps/src/runtime.rs::tests::a_same_runtime_pair_is_refused_for_an_rxo_policy_and_counted_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.total_count_change

**Spec:** §2.2.4.1, S. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for
OfferedIncompatibleQosStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.last_policy_id

**Spec:** §2.2.4.1, S. 130 — "The PolicyId_t of one of the policies that was
found to be incompatible the last time an incompatibility was detected."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_offered_incompatible`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.policies

**Spec:** §2.2.4.1, S. 130 — "A list containing for each policy the total number
of times that the concerned DataWriter discovered a DataReader for the same Topic
with a requested QoS that is incompatible with that offered by the DataWriter."

**Repo:** `crates/dcps/src/status.rs` (`bump_policy_count`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_ordered_access_unoffered_rejects_on_both_sides`,
`crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`

**Status:** done — wie beim Reader nur die erste unverträgliche Policy je Paar.

### §2.2.4.1 PublicationMatchedStatus.total_count

**Spec:** §2.2.4.1, S. 130 — "Total cumulative count the concerned DataWriter
discovered a “match” with a DataReader. That is, it found a DataReader for the
same Topic with a requested QoS that is compatible with that offered by the
DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.total_count_change

**Spec:** §2.2.4.1, S. 130 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for PublicationMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.last_subscription_handle

**Spec:** §2.2.4.1, S. 130 — "Handle to the last DataReader that matched the
DataWriter causing the status to change."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done — nach einer Aufhebung nennt das Feld den Reader, der ging.

### §2.2.4.1 PublicationMatchedStatus.current_count

**Spec:** §2.2.4.1, S. 130 — "The number of DataReaders currently matched to the
concerned DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.current_count_change

**Spec:** §2.2.4.1, S. 130 — "The change in current_count since the last time
the listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for PublicationMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/src/status.rs::tests::an_acknowledgement_keeps_what_came_after_the_report`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.total_count

**Spec:** §2.2.4.1, S. 130 — "Total cumulative count the concerned DataReader
discovered a “match” with a DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.total_count_change

**Spec:** §2.2.4.1, S. 130 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for SubscriptionMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.last_publication_handle

**Spec:** §2.2.4.1, S. 130 — "Handle to the last DataWriter that matched the
DataReader causing the status to change."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.current_count

**Spec:** §2.2.4.1, S. 130 — "The number of DataWriters currently matched to the
concerned DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.current_count_change

**Spec:** §2.2.4.1, S. 130 — "The change in current_count since the last time
the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::subscription_matched_current_count_change_is_read_and_reset`

**Status:** done — der Test liest `current_count_change` nach einer Zuordnung (1), nach dem Verlust (-1) und für eine Zuordnung samt Verlust vor dem Lesen (0).

### §2.2.4.1 QosPolicyCount — policy_id und count

**Spec:** §2.2.4.1, S. 127 — Figure 2.13: QosPolicyCount mit
`policy_id : QosPolicyId_t` und `count : long`.

**Repo:** `crates/dcps/src/status.rs` (`QosPolicyCount`)

**Tests:** `crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`

**Status:** done

### §2.2.4.2 StatusChangedFlag je Status

**Spec:** §2.2.4.2, S. 130–131 — "Associated with each one of an Entity’s
communication status is a logical StatusChangedFlag. This flag indicates whether
that particular communication status has changed since the last time the status
was ‘read’ by the application … Note that Figure 2.14 is only conceptual".

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::unread`: das Flag aus den
Änderungsfeldern); `crates/dcps/src/entity.rs` (`EntityState::status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`

**Status:** done

### §2.2.4.2.1 Einfache Status — Anfangswert FALSE, TRUE bei Änderung

**Spec:** §2.2.4.2.1, S. 131 — "For the plain communication status, the
StatusChangedFlag flag is initially set to FALSE. It becomes TRUE whenever the
plain communication status changes".

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::update`,
`StatusLedger::unread`); `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`:
`set_status_bits`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done

### §2.2.4.2.1 Einfache Status — FALSE nach get_<status>

**Spec:** §2.2.4.2.1, S. 131 — "it is reset to FALSE each time the application
accesses the plain communication status via the proper get_<plain communication
status> operation on the Entity."

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::take`); die
`get_*_status`-Operationen (`clear_status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`

**Status:** done

### §2.2.4.2.1 Einfache Status — FALSE beim Listener-Aufruf

**Spec:** §2.2.4.2.1, S. 131 — "The communication status is also reset to FALSE
whenever the associated listener operation is called as the listener implicitly
accesses the status which is passed as a parameter to the operation."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`: `acknowledge`,
`clear_status_changes` nach dem Aufruf)

**Tests:** `crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`,
`crates/dcps/src/status.rs::tests::a_read_between_the_look_and_the_acknowledgement_is_not_taken_off_twice`

**Status:** done

### §2.2.4.2.1 Einfache Status — Rücksetzen vor dem Listener-Aufruf

**Spec:** §2.2.4.2.1, S. 131 — "The fact that the status is reset prior to
calling the listener means that if the application calls the get_<plain
communication status> from inside the listener it will see the status already
reset."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`,
`WriterListenerChain::takes`, `ReaderListenerChain::takes`)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::the_status_is_read_before_the_listener_runs`

**Status:** `done` — nimmt ein Listener den Status, wird er vor dem Aufruf
quittiert und das Flag gesenkt; ein `get_*_status` im Listener sieht ihn
zurückgesetzt.

### §2.2.4.2.1 Einfache Status — nil-Listener setzt nicht zurück

**Spec:** §2.2.4.2.1, S. 131 — "An exception to this rule is when the associated
listener is the ‘nil’ listener. As described in 2.2.4.3.1 the ‘nil’ listener is
treaded as a NOOP and the act of calling the ‘nil’ listener does not reset the
communication status."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` mit nicht leerer Maske setzt `NilListener`: die Status der Maske werden ohne Wirkung behandelt, gehen nicht nach oben (bei DATA_ON_READERS folgt kein `on_data_available`) und werden nicht zurückgesetzt. `get_listener` liefert dafür keinen. Eine leere Maske ist kein Listener. Die C-ABI (gepollte Listener) nimmt NULL mit Maske ebenso als nil-Listener: `zerodds_poll_listeners` lässt in den Ebenen Publisher, Subscriber und Participant die Status weg, die ein nil-Listener von Writer, Reader, Subscriber oder Topic darunter in seiner Maske hat (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.4.2.1 Beispiel REQUESTED_DEADLINE_MISSED

**Spec:** §2.2.4.2.1, S. 131–132 — "For example, the value of the
StatusChangedFlag associated with the REQUESTED_DEADLINE_MISSED status will
become TRUE each time new deadline occurs … The value changes to FALSE when the
application accesses the status via the corresponding
get_requested_deadline_missed_status method".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel zur Regel oben.

### §2.2.4.2.2 Lese-Status — Anfangswert FALSE

**Spec:** §2.2.4.2.2, S. 132 — "For the read communication status, the
StatusChangedFlag flag is initially set to FALSE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::install_data_available_probe`,
`Subscriber::new`: Probe für DATA_ON_READERS)

**Tests:** `crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done

### §2.2.4.2.2 Lese-Status — TRUE bei neuen Daten

**Spec:** §2.2.4.2.2, S. 132 — "The StatusChangedFlag becomes TRUE when either a
data-sample arrives … The arrival of new data."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::install_data_available_probe`:
Kanal, Eingang, NOT_READ-Samples im Cache)

**Tests:** `crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/data_available_live.rs::a_live_readers_listener_hears_samples_nobody_waited_for`

**Status:** done

### §2.2.4.2.2 Lese-Status — TRUE bei dispose

**Spec:** §2.2.4.2.2, S. 132 — "The arrival of the notification that an instance
has been disposed by: the DataWriter that owns it if OWNERSHIP QoS
kind=EXLUSIVE, or by any DataWriter if OWNERSHIP QoS kind=SHARED."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`:
Marke ohne Daten, NOT_READ)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** done — unter SHARED legt jedes dispose eine NOT_READ-Marke in den Cache, aus der die Probe DATA_AVAILABLE meldet; unter EXCLUSIVE wird ein dispose eines Writers, der die Instanz nicht besitzt, nicht übernommen (§2.2.2.5.1.3).

### §2.2.4.2.2 Lese-Status — TRUE bei Verlust der Liveliness des einzigen Writers

**Spec:** §2.2.4.2.2, S. 132 — "The loss of liveliness of the DataWriter of an
instance for which there is no other DataWriter."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`, `ReaderNews`);
`crates/dcps/src/instance_tracker.rs` (`would_leave_writerless`)

**Tests:**
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/liveliness_qos.rs::linux::a_lost_writer_raises_data_available_before_any_read`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::a_writer_that_leaves_raises_data_available_before_any_read`

**Status:** `done` — die Marke NOT_ALIVE_NO_WRITERS hebt DATA_AVAILABLE vor dem
nächsten `read`/`take`, auch nach dem Wegfall der Zuordnung.

### §2.2.4.2.2 Lese-Status — TRUE bei unregister des einzigen Writers

**Spec:** §2.2.4.2.2, S. 132 — "The arrival of the notification that an instance
has been unregistered by the only DataWriter that is known to be writing the
instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::unregister`)

**Tests:** `crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`

**Status:** done

### §2.2.4.2.2 DATA_AVAILABLE — FALSE nach on_data_available oder read/take

**Spec:** §2.2.4.2.2, S. 132 — "The DATA_AVAILABLE StatusChangedFlag becomes
FALSE when either the corresponding listener operation (on_data_available) is
called or the read or take operation (or their variants) is called on the
associated DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderNews`,
`ReaderStatus::note_read_or_take`, `poll_data_available`)

**Tests:**
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/status_reads.rs::a_read_of_one_instance_lowers_data_available`,
`crates/dcps/tests/status_reads.rs::an_on_data_available_call_lowers_data_available`

**Status:** `done` — ein Flag: der Aufruf von `on_data_available` oder ein
`read`/`take` jeder Variante setzt es zurück, neue Daten setzen es.

### §2.2.4.2.2 DATA_AVAILABLE — Listener-Aufruf und Varianten von read/take

**Spec:** §2.2.4.2.2, S. 132 — "becomes FALSE when either the corresponding
listener operation (on_data_available) is called or the read or take operation
(or their variants) is called".

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderNews`,
`ReaderStatus::note_read_or_take`)

**Tests:**
`crates/dcps/tests/status_reads.rs::a_read_of_one_instance_lowers_data_available`,
`crates/dcps/tests/status_reads.rs::an_on_data_available_call_lowers_data_available`

**Status:** `done` — nach `on_data_available` und nach `read_instance` ist das
Flag FALSE, auch wenn andere Instanzen NOT_READ-Samples haben.

### §2.2.4.2.2 DATA_ON_READERS — FALSE nach on_data_on_readers, on_data_available oder read/take

**Spec:** §2.2.4.2.2, S. 132 — "The DATA_ON_READERS StatusChangedFlag becomes
FALSE when any of the following events occurs: The corresponding listener
operation (on_data_on_readers) is called. The on_data_available listener
operation is called on any DataReader belonging to the Subscriber. The read or
take operation (or their variants) is called on any DataReader belonging to the
Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`SubscriberInner::data_on_readers`,
`cover_data_on_readers`)

**Tests:**
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/status_reads.rs::a_take_on_one_reader_lowers_data_on_readers`

**Status:** done — die drei Ereignisse setzen das Flag zurück. Die GROUP-Freigabe nach einem Listener-Aufruf steht als eigenes Item unten.

### §2.2.4.2.2 DATA_ON_READERS — GROUP-Freigabe nach einem Listener-Aufruf

**Spec:** §2.2.4.2.2, S. 132 — "The DATA_ON_READERS StatusChangedFlag becomes
FALSE when any of the following events occurs: … The on_data_available listener
operation is called on any DataReader belonging to the Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`NewsMarks::cover_held`, `NewsMarks::released`,
`ReaderInner::accept_group_released`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::a_group_release_after_a_listener_call_is_no_news_for_what_it_covered`

**Status:** `done` — ein Listener-Aufruf merkt sich, wie viele Samples die Gruppen-Ordnung des Subscribers für den Reader hielt; deren Freigabe in den Eingang zählt als abgedeckt, ein nicht abgedecktes Sample hebt beide Flags. Der Test prüft das an den Marken eines Offline-Readers; der Weg über eine laufende GROUP-Publisher-Ordnung ist nicht gemessen.

### §2.2.4.2.2 DATA_ON_READERS — Rücksetzen durch Listener und andere Reader

**Spec:** §2.2.4.2.2, S. 132 — "The read or take operation (or their variants) is
called on any DataReader belonging to the Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs`
(`SubscriberInner::cover_data_on_readers`)

**Tests:**
`crates/dcps/tests/status_reads.rs::a_take_on_one_reader_lowers_data_on_readers`

**Status:** `done` — `on_data_on_readers`, `on_data_available` und `read`/`take`
auf einem Reader des Subscribers setzen es zurück, auch wenn ein anderer Reader
NOT_READ-Samples hält.

### §2.2.4.3 Listener je Entity-Art

**Spec:** §2.2.4.3, S. 133 — "All Entity support a listener, which type of which
is specialized to the specific type of the related Entity (e.g.,
DataReaderListener for the DataReader). Listeners are interfaces that the
application must implement." Figure 2.17.

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener`, `DataWriterListener`,
`PublisherListener`, `DataReaderListener`, `SubscriberListener`,
`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datareader_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — die Methoden haben leere Standardrümpfe; eine Anwendung
implementiert nur, was sie braucht.

### §2.2.4.3 Listener zustandslos, Parameter verweist auf die Entity

**Spec:** §2.2.4.3, S. 134 — "Listeners are stateless. It is thus possible to
share the same DataReaderListener instance among all the DataReader objects …
Consequently, the provided parameter contains a reference to the actual
concerned Entity."

**Repo:** `crates/dcps/src/listener.rs` (`*_entity`), `crates/dcps/src/listener_dispatch.rs`

**Tests:** `crates/dcps/tests/listener_entity.rs::a_shared_listener_is_called_with_the_entity`, `crates/dcps/tests/listener_entity.rs::the_bubbled_up_call_gets_the_entity`, `crates/dcps/tests/listener_entity.rs::a_listener_of_the_handle_form_is_still_called`

**Status:** done — jede Operation der sechs Listener-Traits hat eine Entity-Form (`on_data_available_entity(&AnyDataReader)` und die übrigen); der Dispatcher ruft sie mit der Entity, deren Handle er hat, und ihre Voreinstellung ruft die Handle-Form, so dass bestehende Listener wie vorher gerufen werden. Ein geteilter Listener erreicht über die Entity den Reader (`typed::<T>()`).

### §2.2.4.3.1 on_<status> mit dem Status als Parameter

**Spec:** §2.2.4.3.1, S. 134 — "For each communication status, there is a
corresponding operation whose name is on_<communication_status>, which takes a
parameter of type <communication_status> as listed in 2.2.4.1".

**Repo:** `crates/dcps/src/listener.rs`

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::liveliness_changed_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::requested_incompatible_qos_dispatcher_fires_listener`,
`crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`

**Status:** done

### §2.2.4.3.1 on_<status> auch an den umgebenden Entities

**Spec:** §2.2.4.3.1, S. 134–135 — "on_<communication_status> is available on the
relevant Entity as well as those that embed it, as expressed in the following
figure" (Figure 2.18: Domain; Publisher, Topic, Subscriber; DataWriter,
DataReader).

**Repo:** `crates/dcps/src/listener.rs` (`PublisherListener` mit den
DataWriter-Callbacks, `SubscriberListener` mit den DataReader-Callbacks,
`DomainParticipantListener` mit allen)

**Tests:** `crates/dcps/src/listener.rs::tests::publisher_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::subscriber_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`

**Status:** done

### §2.2.4.3.1 Maske beim Anhängen

**Spec:** §2.2.4.3.1, S. 135 — "When the application attaches a listener on an
entity, it must set a mask that indicates to the middleware which operations are
enabled within this listener (see operation Entity::set_listener)."

**Repo:** `set_listener(listener, mask)` je Entity

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_with_only_data_available_bit_does_not_call_reader_for_others`,
`crates/dcps/src/listener_dispatch.rs::tests::writer_with_only_publication_matched_bit_passes_other_events_up`

**Status:** done

### §2.2.4.3.1 Der spezifischste freigegebene Listener

**Spec:** §2.2.4.3.1, S. 135 — "When a plain communication status changes, the
middleware triggers the most ‘specific’ relevant listener operation that is
enabled." Fußnote 30: DataWriter, sonst Publisher, sonst DomainParticipant.

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`dispatch_*`,
`WriterListenerChain`, `ReaderListenerChain`, `TopicListenerChain`)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::publication_matched_consumed_at_writer`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::sample_lost_bubbles_to_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_bubbles_to_participant`

**Status:** done

### §2.2.4.3.1 nil-Listener als NO-OP

**Spec:** §2.2.4.3.1, S. 135 — "In case the most specific relevant listener
operation corresponds to an application-installed ‘nil’ listener the operation
will be considered handled by a NO-OP operation."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` mit nicht leerer Maske setzt `NilListener`: die Status der Maske werden ohne Wirkung behandelt, gehen nicht nach oben (bei DATA_ON_READERS folgt kein `on_data_available`) und werden nicht zurückgesetzt. `get_listener` liefert dafür keinen. Eine leere Maske ist kein Listener. Die C-ABI (gepollte Listener) nimmt NULL mit Maske ebenso als nil-Listener: `zerodds_poll_listeners` lässt in den Ebenen Publisher, Subscriber und Participant die Status weg, die ein nil-Listener von Writer, Reader, Subscriber oder Topic darunter in seiner Maske hat (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.4.3.1 Default-Verhalten am DomainParticipant

**Spec:** §2.2.4.3.1, S. 135 — "This behavior allows the application to set a
default behavior (e.g., in the listener associated with the DomainParticipant)
and setting dedicated behaviors only where needed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Regel oben.

### §2.2.4.3.2 Lese-Status — zuerst on_data_on_readers

**Spec:** §2.2.4.3.2, S. 135 — "first, the middleware tries to trigger the
SubscriberListener operation on_data_on_readers with a parameter of the related
Subscriber; if this does not succeed (no listener or operation non-enabled), it
tries to trigger on_data_available on all the related DataReaderListener
objects, with as parameter the related DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::poll_data_available`);
`crates/dcps/src/listener_dispatch.rs` (`dispatch_data_on_readers`,
`dispatch_data_available`)

**Tests:** `crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::data_on_readers_does_not_call_reader_stage`

**Status:** done — `on_data_on_readers` geht vom Subscriber an den
DomainParticipant weiter, `on_data_available` vom Reader an Subscriber und
DomainParticipant (`data_on_readers_bubbles_to_participant`,
`data_available_bubbles_all_the_way_to_participant`). Die Spec nennt hier nur
SubscriberListener und DataReaderListener; der Widerspruch ist in der
Befunddatei geführt.

### §2.2.4.3.2 Kein on_data_available nach on_data_on_readers, notify_datareaders

**Spec:** §2.2.4.3.2, S. 136 — "Note that if on_data_on_readers is called, then
the middleware will not try to call on_data_available, however, the application
can force a call to the DataReader objects that have data by means of the
notify_datareaders operation."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::poll_data_available`,
`ReaderStatus::notify_data_available`, `data_available_pending`)

**Tests:** `crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`,
`crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`

**Status:** done

### §2.2.4.3.2 Begründung der zwei Wege

**Spec:** §2.2.4.3.2, S. 136 — "The rationale is that either the application is
interested in relations among data arrivals and it must use the first option …
or it wants to treat each DataReader fully independently and it may choose the
second option".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung.

### §2.2.4.3.2 Keine Ereigniswarteschlange für Listener

**Spec:** §2.2.4.3.2, S. 136 — "There is no implied “event queuing” in the
invocation of the listeners in the sense that, if several changes of status of
the same kind occur in sequence, it is not necessary that the DCPS
implementation delivers one listener callback per “unit” change … as long as the
LivelinessChangedStatus provided to the listener corresponds to the most current
one."

**Repo:** `crates/dcps/src/listener_dispatcher.rs` (ein Durchlauf für eine Folge
von Änderungen)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::a_look_asked_for_during_a_look_runs_after_it_not_inside_it`,
`crates/dcps/src/listener_dispatch.rs::tests::a_run_of_changes_is_handed_over_once_with_the_most_current_status`

**Status:** done — eine Folge von vier Änderungen erreicht den Listener in einem Aufruf mit dem aktuellsten LivelinessChangedStatus; eine Änderung danach ist der nächste Aufruf mit ihren eigenen Änderungszählern.

### §2.2.4.4 Ablauf mit Conditions und WaitSets

**Spec:** §2.2.4.4, S. 136–138 — "The application indicates which relevant
information it wants to get, by creating Condition objects … and attaching them
to a WaitSet. It then waits on that WaitSet until the trigger_value of one or
several Condition objects become TRUE. It then uses the result of the wait …
Usually the first step is done in an initialization phase".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nutzungsmuster; die Operationen sind unter
§2.2.2.1.6 bis §2.2.2.1.9 und §2.2.2.5.8 bis §2.2.2.5.9 geführt.

### §2.2.4.4 Conditions einer Entity gehören zu genau einer Entity

**Spec:** §2.2.4.4, S. 138 — "Condition objects are meant to embed all that is
needed to react properly when enabled. In particular, Entity-related conditions
are related to exactly one Entity and cannot be shared."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition`: ein `EntityState`);
`crates/dcps/src/condition.rs` (`ReadCondition`: ein Reader)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`,
`crates/dcps/src/entity.rs::tests::status_condition_trigger_value`

**Status:** done

### §2.2.4.4 WaitSet — BLOCKED und UNBLOCKED

**Spec:** §2.2.4.4, S. 138 — "If the wait operation is called on WaitSet with
state BLOCKED, it will block the calling thread. If wait is called on a WaitSet
with state UNBLOCKED, it will return immediately. In addition, when the WaitSet
transitions from BLOCKED to UNBLOCKED it wakes up any threads that had called
wait on it." Figure 2.20.

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`, `WaitSet::poll_active`,
`signal_data_available`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_returns_immediately_if_already_triggered`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`,
`crates/dcps/src/condition.rs::tests::data_signal_wakes_a_blocked_wait`,
`crates/dcps/src/condition.rs::tests::a_signal_between_check_and_block_is_not_lost`

**Status:** done — Datenankunft weckt sofort; eine GuardCondition oder
StatusCondition wird nach höchstens `poll_interval` (1 ms) gesehen (siehe
§2.2.2.1.8.1). Es wartet höchstens ein Thread (§2.2.2.1.6.3).

### §2.2.4.4 Keine Ereigniswarteschlange für WaitSets

**Spec:** §2.2.4.4, S. 138 — "there is no implied “event queuing” in the
awakening of a WaitSet in the sense that, if several Conditions attached to the
WaitSet have their trigger_value transition to TRUE in sequence the DCPS
implementation needs to only unblock the WaitSet once."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §2.2.4.4.1 trigger_value der StatusCondition

**Spec:** §2.2.4.4.1, S. 139 — "The trigger_value of a StatusCondition is the
Boolean OR of the ChangedStatusFlag of all the communication statuses to which it
is sensitive. That is, trigger_value==FALSE only if all the values of the
ChangedStatusFlags are FALSE. The sensitivity … is controlled by the list of
enabled_statuses set on the condition by means of the set_enabled_statuses
operation."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value`:
`status_changes() & enabled_statuses`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done — für DATA_AVAILABLE und DATA_ON_READERS gelten die
Abweichungen unter §2.2.4.2.2.

### §2.2.4.5 trigger_value der ReadCondition

**Spec:** §2.2.4.5, S. 139 — "the trigger_value of the ReadCondition is tied to
the presence of at least a sample managed by the Service with SampleState,
ViewState, and InstanceState matching those of the ReadCondition. Furthermore,
for the QueryCondition to have a trigger_value==TRUE, the data associated with
the sample must be such that the query_expression evaluates to TRUE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`,
`DataReader::create_querycondition`: Trigger über die Samples im Cache)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`,
`crates/dcps/tests/reader_conditions.rs::a_waitset_returns_a_read_condition_of_a_reader_with_data`

**Status:** done — ViewState und InstanceState sind die bei der Aufnahme
festgehaltenen Werte (siehe §2.2.2.5.1).

### §2.2.4.5 Ein take ändert mehrere Conditions

**Spec:** §2.2.4.5, S. 139 — "a single take operation can potentially change the
trigger_value of several ReadCondition or QueryCondition conditions. For
example, if all samples are taken, any ReadCondition and QueryCondition
conditions associated with the DataReader that had their trigger_value==TRUE
before will see the trigger_value change to FALSE."

**Repo:** `crates/dcps/src/subscriber.rs` (Trigger lesen den Cache bei jeder
Abfrage)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`

**Status:** done

### §2.2.4.5 Kein Zurücknehmen eines Weckens

**Spec:** §2.2.4.5, S. 139 — "Once we have trigger_value==TRUE on a condition it
may wake up the attached WaitSet, the condition transitioning to
trigger_value==FALSE does not necessarily ‘unwakeup’ the WaitSet … an
application blocked on a WaitSet may return from the wait with a list of
conditions some of which are no longer “active.”"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §2.2.4.5 Beispiel sample_state_mask {NOT_READ} und {READ, NOT_READ}

**Spec:** §2.2.4.5, S. 139 — "A ReadCondition that has a sample_state_mask =
{NOT_READ} will have trigger_value of TRUE whenever a new sample arrives and will
transition to FALSE as soon as all the newly-arrived samples are either read …
or taken … However if the same ReadCondition had a sample_state_mask = {READ,
NOT_READ}, then the trigger_value would only become FALSE once all the
newly-arrived samples are taken".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`

**Status:** done

### §2.2.4.5.1 trigger_value der GuardCondition

**Spec:** §2.2.4.5.1, S. 139 — "The trigger_value of a GuardCondition is
completely controlled by the application via operation set_trigger_value."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::set_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`

**Status:** done

### §2.2.4.6 Kombination — erst Listener, dann WaitSet

**Spec:** §2.2.4.6, S. 139 — "Those two mechanisms may be combined in the
application … if both mechanisms are enabled, then the listener mechanism is used
first and then the WaitSet objects are signalled."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::notify_data_arrived`);
`crates/dcps/src/runtime.rs` (`signal_reader_data`)

**Tests:**
`crates/dcps/tests/waitset_notification.rs::a_listener_and_a_waitset_both_see_the_same_arrival`,
`crates/dcps/tests/listener_before_waitset.rs::the_listener_runs_before_the_waitset_is_signalled`

**Status:** `done` — auf beiden Pfaden läuft der Listener zuerst. Bei einem
einfachen Status setzt der Listener-Aufruf das Flag zurück (§2.2.4.2.1); eine
StatusCondition auf diesem Status löst danach nicht aus. Der Widerspruch ist in
der Befunddatei geführt.

### §2.2.4.6 Kombination — Reihenfolge auf dem Empfangspfad

**Spec:** §2.2.4.6, S. 139 — "the listener mechanism is used first and then the
WaitSet objects are signalled."

**Repo:** `crates/dcps/src/runtime.rs::signal_reader_data`

**Tests:**
`crates/dcps/tests/listener_before_waitset.rs::the_listener_runs_before_the_waitset_is_signalled`

**Status:** `done` — `signal_reader_data` ruft erst den Listener und weckt
danach die WaitSets.

---

## §2.2.5 Built-in Topics

### §2.2.5 Zweck der Built-in-Topics

**Spec:** §2.2.5, S. 140 — "the DCPS specification introduces a set of built-in
topics and corresponding DataReader objects that can then be used by the
application. The information is then accessed as if it was normal application
data. This approach avoids introducing a new API to access this information".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung; die Anforderungen stehen in den
folgenden Items.

### §2.2.5 Built-in-Reader in einem Built-in-Subscriber

**Spec:** §2.2.5, S. 140 — "The built-in data-readers all belong to a built-in
Subscriber. This subscriber can be retrieved by using the method
get_builtin_subscriber provided by the DomainParticipant."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::new`:
die vier Reader); `crates/dcps/src/participant.rs`
(`DomainParticipant::get_builtin_subscriber`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::get_builtin_subscriber_returns_subscriber`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/src/builtin_subscriber.rs::tests::builtin_subscriber_has_four_readers`

**Status:** done — ein Built-in-Subscriber je DomainParticipant, beide Aufrufe
liefern dasselbe Objekt.

### §2.2.5 Built-in-Subscriber als Subscriber

**Spec:** §2.2.5, S. 140 — "The built-in data-readers all belong to a built-in
Subscriber."; §2.2.2.2.1.13, S. 39 — "This operation allows access to the
built-in Subscriber."

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::the_builtin_subscriber_is_a_subscriber`

**Status:** done — `BuiltinSubscriber` leitet an seinen `Subscriber` weiter: QoS, Listener, StatusCondition und die übrigen Operationen sind seine eigenen, seine Reader sind die vier Built-in-Reader (`lookup_datareader`, `get_datareaders`), er ist freigegeben, und die Anwendung kann sie nicht löschen (`PreconditionNotMet`).

### §2.2.5 lookup_datareader mit dem Topic-Namen

**Spec:** §2.2.5, S. 140 — "The built-in DataReader objects can be retrieved by
using the operation lookup_datareader, with the Subscriber and the topic name as
parameters."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::lookup_datareader`,
`BuiltinTopic`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::lookup_datareader_routes_by_type`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — die vier Namen liefern ihren Reader; ein unbekannter Name oder ein Typ, der nicht der des Topics ist, liefert nil (`None`).

### §2.2.5 lookup_datareader — unbekannter Name

**Spec:** §2.2.2.5.2.7, S. 80 — lookup_datareader: "If no such DataReader exists,
the operation will return ’nil.’"

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — nil (`None`) statt `BadParameter`.

### §2.2.5 QoS der Built-in-Entities — nicht festgelegte Werte

**Spec:** §2.2.5, S. 140 — USER_DATA, TOPIC_DATA, GROUP_DATA, PARTITION
"<unspecified>"; LATENCY_BUDGET "duration = <unspecified>"; LIVELINESS
"lease_duration = <unspecified>".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Vorgabe.

### §2.2.5 QoS der Built-in-Reader — DURABILITY TRANSIENT_LOCAL

**Spec:** §2.2.5, S. 140 — "DURABILITY TRANSIENT_LOCAL".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS der Built-in-Reader — DURABILITY_SERVICE

**Spec:** §2.2.5, S. 140 — "DURABILITY_SERVICE Does not apply as DURABILITY is
TRANSIENT_LOCAL".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Vorgabe.

### §2.2.5 QoS des Built-in-Subscribers — PRESENTATION

**Spec:** §2.2.5, S. 140 — "PRESENTATION access_scope = TOPIC coherent_access =
FALSE ordered_access = FALSE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_subscriber_qos`)

**Tests:**
`crates/dcps/src/builtin_subscriber.rs::tests::the_builtin_subscriber_presents_by_topic`

**Status:** `done` — `access_scope` TOPIC, `coherent_access` und
`ordered_access` FALSE.

### §2.2.5 QoS der Built-in-Reader — DEADLINE unendlich

**Spec:** §2.2.5, S. 140 — "DEADLINE Period = infinite".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — OWNERSHIP SHARED

**Spec:** §2.2.5, S. 140 — "OWNERSHIP SHARED".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — LIVELINESS AUTOMATIC

**Spec:** §2.2.5, S. 140 — "LIVELINESS kind = AUTOMATIC".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — TIME_BASED_FILTER 0

**Spec:** §2.2.5, S. 140 — "TIME_BASED_FILTER minimum_separation = 0".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — RELIABILITY RELIABLE

**Spec:** §2.2.5, S. 140 — "RELIABILITY kind = RELIABLE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS der Built-in-Reader — max_blocking_time 100 ms

**Spec:** §2.2.5, S. 140 — "RELIABILITY … max_blocking_time = 100 milliseconds".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — DESTINATION_ORDER BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.5, S. 140 — "DESTINATION_ORDER BY_RECEPTION_TIMESTAMP".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — HISTORY KEEP_LAST 1

**Spec:** §2.2.5, S. 140 — "HISTORY kind = KEEP_LAST depth = 1".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS der Built-in-Reader — RESOURCE_LIMITS unbegrenzt

**Spec:** §2.2.5, S. 140 — "RESOURCE_LIMITS All LENGTH_UNLIMITED."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS der Built-in-Reader — READER_DATA_LIFECYCLE unendlich

**Spec:** §2.2.5, S. 140 — "READER_DATA_LIFECYCLE
autopurge_nowriter_samples_delay = infinite autopurge_disposed_samples_delay =
infinite".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 QoS des Built-in-Subscribers — ENTITY_FACTORY TRUE

**Spec:** §2.2.5, S. 140 — "ENTITY_FACTORY autoenable_created_entities = TRUE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — der Wert ist an allen vier Built-in-Readern (bzw. am Built-in-Subscriber) geprüft.

### §2.2.5 Listener des Built-in-Subscribers und der Built-in-Topics

**Spec:** §2.2.5, S. 141 — "The built-in Subscriber and all of its built-in
Topics have nil listeners with all statuses appearing in their listener masks."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::new`); `crates/dcps/src/participant.rs` (`attach_participant` for the built-in Subscriber)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::a_builtin_status_does_not_reach_the_participant_listener`

**Status:** done — `BuiltinSubscriber::new` setzt den nil-Listener mit allen Status auf den Built-in-Subscriber und hängt ihn an seinen Participant; ein Status eines Built-in-Readers endet dort und erreicht den Participant-Listener nicht. Die Built-in-Topics sind eigenständige Objekte ohne Participant-Kette.

### §2.2.5 Listener der Built-in-Reader

**Spec:** §2.2.5, S. 141 — "The built-in DataReaders have nil listeners with no
statuses in their masks."

**Repo:** `crates/dcps/src/builtin_subscriber.rs`

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::a_builtin_status_does_not_reach_the_participant_listener`

**Status:** done — die Built-in-Reader haben keinen Listener (ein nil-Listener mit leerer Maske ist keiner); `get_listener` liefert keinen.

### §2.2.5 QoS der entfernten Entity als Datenfelder

**Spec:** §2.2.5, S. 141 — "The information that is accessible about the remote
entities by means of the built-in topics includes all the QoS policies that
apply to the corresponding remote Entity. This QoS policies appear as normal
‘data’ fields inside the data read by means of the built-in Topic."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Felder sind je Built-in-Topic unten
geführt.

### §2.2.5 Keine Daten über Entities des eigenen DomainParticipant

**Spec:** §2.2.5, S. 141 — "A built-in DataReader obtained from a given
Participant will not provide data pertaining to Entities created from that same
Participant under the assumption that such entities are already known to the
application that created them."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_spdp_datagram`: eigener Beacon
verworfen)

**Tests:** `crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_skips_self_beacon`,
`crates/dcps/tests/builtin_no_own_data.rs::the_own_entities_are_in_no_built_in_reader`

**Status:** done — weder der Participant noch seine Topics, Writer und Reader erscheinen in seinen eigenen Built-in-Readern.

### §2.2.5 Keine Daten über eigene Topics, DataWriter und DataReader

**Spec:** §2.2.5, S. 141 — "will not provide data pertaining to Entities created
from that same Participant".

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_no_own_data.rs::the_own_entities_are_in_no_built_in_reader`

**Status:** done — siehe das Item darüber.

### §2.2.5 DCPSParticipant — key

**Spec:** §2.2.5, S. 141 — DCPSParticipant "(entry created when a
DomainParticipant object is created)": "key BuiltinTopicKey_t DCPS key to
distinguish entries."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`ParticipantBuiltinTopicData::key`,
`ParticipantBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::participant_from_wire_strips_to_guid`,
`crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_pushes_into_builtin_participant_reader`

**Status:** done — der Schlüssel ist der GUID des Teilnehmers.

### §2.2.5 DCPSParticipant — user_data

**Spec:** §2.2.5, S. 141 — "user_data UserDataQosPolicy Policy of the
corresponding DomainParticipant."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::participant_sample_carries_user_data`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSParticipant — instance_handle im SampleInfo

**Spec:** §2.2.2.2.1.14, S. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples available for the built-in
DataReader to the “DCPSParticipant” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`), `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — die Built-in-Reader vergeben `InstanceHandle::from_guid`
des Schlüssels statt eines Zählers.

### §2.2.5 DCPSTopic — key

**Spec:** §2.2.5, S. 141 — DCPSTopic "(entry created when a Topic object is
created)": "key BuiltinTopicKey_t DCPS key to distinguish entries."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::synthesize_key`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_synthesize_key_is_stable_and_distinct`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::two_participants_creating_one_topic_are_two_dcps_topic_entries`

**Status:** done — der Schlüssel wird aus Participant, Topic- und Typname gebildet und unterscheidet die Einträge; zwei Participants, die dasselbe Topic anlegen, ergeben zwei Einträge.

### §2.2.5 DCPSTopic — Eintrag je Topic-Objekt und Werte des Topics

**Spec:** §2.2.5, S. 141 — "(entry created when a Topic object is created)";
die Policies sind je "Policy of the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::synthesize_key`), `crates/dcps/src/runtime.rs` (`push_sedp_events_to_builtin_readers`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::two_participants_creating_one_topic_are_two_dcps_topic_entries`, `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — ein Eintrag je Participant, Topic und Typ; die Ankündigung über den SEDP-Topics-Writer trägt alle Policies des Topics. Kündigt ein Participant kein Topic über SEDP an (ein Peer eines anderen Herstellers), entsteht der Eintrag aus seinen Endpunkten (`from_publication`, `from_subscription`): die Policies sind dann die des DataWriter oder DataReader, HISTORY, RESOURCE_LIMITS und TRANSPORT_PRIORITY haben ihren Default, weil kein Endpunkt sie ankündigt (DDSI-RTPS 2.5 Tabellen 9.18, 9.19).

### §2.2.5 DCPSTopic — name

**Spec:** §2.2.5, S. 141 — "name string Name of the Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::name`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`,
`crates/dcps/src/builtin_topics.rs::tests::topic_roundtrip`

**Status:** done

### §2.2.5 DCPSTopic — type_name

**Spec:** §2.2.5, S. 141 — "type_name string Name of the type attached to the
Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — durability

**Spec:** §2.2.5, S. 141 — "durability DurabilityQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — durability_service

**Spec:** §2.2.5, S. 141 — "durability_service DurabilityServiceQosPolicy Policy
of the corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — deadline

**Spec:** §2.2.5, S. 141 — "deadline DeadlineQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — latency_budget

**Spec:** §2.2.5, S. 141 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — liveliness

**Spec:** §2.2.5, S. 141 — "liveliness LivelinessQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — reliability

**Spec:** §2.2.5, S. 141 — "reliability ReliabilityQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — transport_priority

**Spec:** §2.2.5, S. 141 — "transport_priority TransportPriorityQosPolicy Policy
of the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TopicBuiltinTopicData::transport_priority`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — für einen aus Endpunkten abgeleiteten Eintrag siehe das
`open`-Item „Eintrag je Topic-Objekt und Werte des Topics“.

### §2.2.5 DCPSTopic — lifespan

**Spec:** §2.2.5, S. 141 — "lifespan LifespanQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — destination_order

**Spec:** §2.2.5, S. 141 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — history

**Spec:** §2.2.5, S. 141 — "history HistoryQosPolicy Policy of the corresponding
Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::history`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — für einen aus Endpunkten abgeleiteten Eintrag siehe das
`open`-Item „Eintrag je Topic-Objekt und Werte des Topics“.

### §2.2.5 DCPSTopic — resource_limits

**Spec:** §2.2.5, S. 141 — "resource_limits ResourceLimitsQosPolicy Policy of
the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TopicBuiltinTopicData::resource_limits`, `TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — für einen aus Endpunkten abgeleiteten Eintrag siehe das
`open`-Item „Eintrag je Topic-Objekt und Werte des Topics“.

### §2.2.5 DCPSTopic — ownership

**Spec:** §2.2.5, S. 141 — "ownership OwnershipQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — topic_data

**Spec:** §2.2.5, S. 141 — "topic_data TopicDataQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSTopic — instance_handle im SampleInfo

**Spec:** §2.2.2.2.1.15, S. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSTopic” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — der Handle ist `InstanceHandle::from_guid` des
synthetischen Schlüssels; `ignore_topic` mit dem Handle aus dem SampleInfo
trennt die Endpoints des Topics.

### §2.2.5 DCPSPublication — key

**Spec:** §2.2.5, S. 142 — "key BuiltinTopicKey_t DCPS key to distinguish
entries."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::key`, `PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done — der Schlüssel ist der GUID des DataWriter.

### §2.2.5 DCPSPublication — participant_key

**Spec:** §2.2.5, S. 142 — "participant_key BuiltinTopicKey_t DCPS key of the
participant to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::participant_key`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — topic_name

**Spec:** §2.2.5, S. 142 — "topic_name string Name of the related Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::topic_name`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** done

### §2.2.5 DCPSPublication — type_name

**Spec:** §2.2.5, S. 142 — "type_name string Name of the type attached to the
related Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::type_name`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — durability

**Spec:** §2.2.5, S. 142 — "durability DurabilityQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::durability`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — durability_service

**Spec:** §2.2.5, S. 142 — "durability_service DurabilityServiceQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — deadline

**Spec:** §2.2.5, S. 142 — "deadline DeadlineQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::deadline`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — latency_budget

**Spec:** §2.2.5, S. 142 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::latency_budget`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — liveliness

**Spec:** §2.2.5, S. 142 — "liveliness LivelinessQosPolicy Policy of the
corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — reliability

**Spec:** §2.2.5, S. 142 — "reliability ReliabilityQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::reliability`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — lifespan

**Spec:** §2.2.5, S. 142 — "lifespan LifespanQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::lifespan`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — user_data

**Spec:** §2.2.5, S. 142 — "user_data UserDataQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::user_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`,
`crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done

### §2.2.5 DCPSPublication — ownership

**Spec:** §2.2.5, S. 142 — "ownership OwnershipQosPolicy Policy of the
corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — ownership_strength

**Spec:** §2.2.5, S. 142 — "ownership_strength OwnershipStrengthQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — destination_order

**Spec:** §2.2.5, S. 142 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — presentation

**Spec:** §2.2.5, S. 142 — "presentation PresentationQosPolicy Policy of the
Publisher to which the DataWriter belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSPublication — partition

**Spec:** §2.2.5, S. 142 — "partition PartitionQosPolicy Policy of the Publisher
to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::partition`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — topic_data

**Spec:** §2.2.5, S. 142 — "topic_data TopicDataQosPolicy Policy of the related
Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::topic_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done — ein DataWriter von ZeroDDS kündigt den eigenen Wert an, falls
gesetzt, sonst den des Topics (§2.2.3.2).

### §2.2.5 DCPSPublication — group_data

**Spec:** §2.2.5, S. 142 — "group_data GroupDataQosPolicy Policy of the
Publisher to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::group_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — instance_handle im SampleInfo

**Spec:** §2.2.2.2.1.16, S. 40 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSPublication” topic."; §2.2.2.5.3.34, S. 97 — "These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSPublications” builtin topic".

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — der Handle ist der aus `DataReader::matched_publications`.

### §2.2.5 DCPSSubscription — key

**Spec:** §2.2.5, S. 143 — "key BuiltinTopicKey_t DCPS key to distinguish
entries."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::key`, `SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done — der Schlüssel ist der GUID des DataReader.

### §2.2.5 DCPSSubscription — participant_key

**Spec:** §2.2.5, S. 143 — "participant_key BuiltinTopicKey_t DCPS key of the
participant to which the DataReader belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::participant_key`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — topic_name

**Spec:** §2.2.5, S. 143 — "topic_name string Name of the related Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — type_name

**Spec:** §2.2.5, S. 143 — "type_name string Name of the type attached to the
related Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — durability

**Spec:** §2.2.5, S. 143 — "durability DurabilityQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — deadline

**Spec:** §2.2.5, S. 143 — "deadline DeadlineQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — latency_budget

**Spec:** §2.2.5, S. 143 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — liveliness

**Spec:** §2.2.5, S. 143 — "liveliness LivelinessQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — reliability

**Spec:** §2.2.5, S. 143 — "reliability ReliabilityQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — ownership

**Spec:** §2.2.5, S. 143 — "ownership OwnershipQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — destination_order

**Spec:** §2.2.5, S. 143 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — user_data

**Spec:** §2.2.5, S. 143 — "user_data UserDataQosPolicy Policy of the
corresponding DataReader."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::user_data`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — time_based_filter

**Spec:** §2.2.5, S. 143 — "time_based_filter TimeBasedFilterQosPolicy Policy of
the corresponding DataReader."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::time_based_filter`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — presentation

**Spec:** §2.2.5, S. 143 — "presentation PresentationQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — partition

**Spec:** §2.2.5, S. 143 — "partition PartitionQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::partition`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — topic_data

**Spec:** §2.2.5, S. 143 — "topic_data TopicDataQosPolicy Policy of the related
Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — group_data

**Spec:** §2.2.5, S. 143 — "group_data GroupDataQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — das Sample trägt den Wert; der Test setzt ihn vom Default weg und liest ihn zurück.

### §2.2.5 DCPSSubscription — instance_handle im SampleInfo

**Spec:** §2.2.2.2.1.17, S. 40 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSSubscription” topic."; §2.2.2.4.2.24, S. 68 — "These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSSubscriptions” builtin topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — der Handle ist der aus `DataWriter::matched_subscriptions`.

## §2.2.6 Interaction Model

### §2.2.6 Zweck der Interaktionsmodelle

**Spec:** §2.2.6, S. 143 — "these models are not intended to explain how the
Service is implemented … should be understood as how it may work rather than how
it actually does work".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Veranschaulichung.

### §2.2.6.1 Publication View

**Spec:** §2.2.6.1, S. 144–145 — Figure 2.21; "It is important to take into
account that the corresponding notifications are now delayed until the last
resume_publications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ablaufdiagramm; `suspend_publications` und
`resume_publications` sind unter §2.2.2.4.1.8 und §2.2.2.4.1.9 geführt.

### §2.2.6.2.1 Notification via Listeners

**Spec:** §2.2.6.2.1, S. 145 — Figure 2.22; "NOTE: When the two kinds of
listeners are set, the SubscriberListener supersedes the DataReaderListener
ones."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ablaufdiagramm; die Regel ist unter
§2.2.4.3.2 geführt.

### §2.2.6.2.2 Notifications via Conditions and Wait-Sets

**Spec:** §2.2.6.2.2, S. 147 — Figure 2.23; "Note that the wait returns the
list of all the enabled conditions, in an arrival cycle".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ablaufdiagramm; `wait` ist unter
§2.2.2.1.6.3 geführt.

---

## §2.3 OMG IDL Platform Specific Model (PSM)

### §2.3.1 Einführung

**Spec:** §2.3.1, S. 149 — "The OMG IDL PSM is provided by means of the IDL that
defines the interface an application can use to interact with the Service."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung. Die Rust-API von `zerodds-dcps`
ist keine Abbildung dieses IDL (siehe Kopf); die folgenden Items prüfen die
Entsprechungen der IDL-Konstanten, -Typen und -Schnittstellen.

### §2.3.2 out-Parameter als inout

**Spec:** §2.3.2, S. 149 — "‘Out’ parameters in the PIM are conventionally
mapped to ‘inout’ parameters in the PSM in order to minimize the memory
allocation performed by the Service".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Regel der IDL-Abbildung; die Rust-API gibt
Ergebnisse als Rückgabewert (`Result<T>`) zurück.

### §2.3.2 Schnittstellen und Datentypen

**Spec:** §2.3.2, S. 149 — "The PIM to PSM mapping maps the UML interfaces and
classes interfaces into IDL interfaces. Plain data types are mapped into
structures. IDL interfaces do not support overloading … the abstract operation
appears inside comments."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Regel der IDL-Abbildung; die Rust-API bildet
Klassen auf Typen und Traits ab, typisierte Operationen auf Generics
(`DataWriter<T>`, `DataReader<T>`).

### §2.3.2 Masken für StatusKind, SampleStateKind, ViewStateKind, InstanceStateKind

**Spec:** §2.3.2, S. 149 — "Enumerations have been mapped into either IDL ’enum’
or hand-picked IDL ‘long’ values that correspond to increasing powers of 2 …
This selection affected only the PIM ‘status kind’ values, namely: StatusKind,
SampleStateKind, ViewStateKind, and InstanceStateKind. The name of the mask type
is formed by replacing the word ‘Kind’ with the word ‘Mask’".

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`, `state`);
`crates/dcps/src/entity.rs` (`StatusMask`); `crates/dcps/src/sample_info.rs`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`,
`crates/dcps/src/psm_constants.rs::tests::instance_state_not_alive_is_or_of_disposed_and_no_writers`

**Status:** `done` — die Bits sind Zweierpotenzen nach der IDL, in
`psm_constants` und in `sample_info` gleich.

### §2.3.2 Sammlungen als Sequenzen

**Spec:** §2.3.2, S. 149 — "Collection parameters have been mapped into IDL
sequences. The only exception applies to the case where the collection elements
are hand-picked IDL ‘long.’ In this case the collection is mapped into an IDL
‘long’ interpreted as a mask."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Regel der IDL-Abbildung; die Rust-API
verwendet `Vec<T>` und `u32`-Masken.

### §2.3.2 QosPolicy als Struktur, QoS je Entity als Struktur

**Spec:** §2.3.2, S. 149 — "Each QosPolicy has been mapped as an IDL struct. The
collection of policies suitable for each Entity has been modeled as another IDL
struct that contains attributes corresponding to the policies that apply to this
Entity."

**Repo:** `crates/qos/src/policies/` (eine Struktur je Policy);
`crates/dcps/src/qos.rs` (`DomainParticipantQos`, `TopicQos`, `PublisherQos`,
`SubscriberQos`, `DataWriterQos`, `DataReaderQos`); `crates/dcps/src/factory.rs`
(`DomainParticipantFactoryQos`)

**Tests:** `crates/qos/src/policies/qos_set.rs::tests::writer_qos_clone_and_eq`,
`crates/dcps/src/qos.rs::group_policy_tests::endpoint_default_presentation_inherits_the_group_value`

**Status:** done — die Inhalte je Struktur stehen unter §2.3.3.

### §2.3.2 Fehler als Rückgabecodes

**Spec:** §2.3.2, S. 149 — "Error-return values have been mapped to plain return
codes of the corresponding functions."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::as_return_code`, `return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`,
`crates/dcps/src/error.rs::tests::rc_not_enabled_maps`

**Status:** done — die Rust-API liefert `Result<T, DdsError>`; `as_return_code`
bildet jeden Fehler auf den Wert des `ReturnCode_t` ab.

### §2.3.2 DataSample nicht abgebildet

**Spec:** §2.3.2, S. 149–150 — "The DataSample class that associates the
SampleInfo and Data collections returned from the data-accessing operations
(read and take) have not been explicitly mapped into IDL … It is anticipated
that additional data-accessing API’s may be provided on each target language".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; die Rust-API liefert Paare aus
Daten und SampleInfo (`Sample<T>`).

### §2.3.2 WaitSet und GuardCondition mit Konstruktor ohne Argumente

**Spec:** §2.3.2, S. 150 — "The classes that do not have factory operations,
namely WaitSet and GuardCondition are mapped to IDL interfaces … the
implementation language mapping should offer at least a constructor that takes
no arguments".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::new`, `GuardCondition::new`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_default_is_empty`,
`crates/dcps/src/condition.rs::tests::guard_condition_starts_false`

**Status:** done

### §2.3.2 get_instance als statische Operation

**Spec:** §2.3.2, S. 150 — "The language implementations of the
DomainParticipantFactory interface should have the static operation get_instance
described in 2.2.2.2.2."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::instance`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done — die Operation heißt `instance` (§2.2.2.2.2.3).

### §2.3.2 Duration_t und Time_t mit sec und nanosec

**Spec:** §2.3.2, S. 150 — "The two types used to represent time: Duration_t and
Time_t are mapped into structures that contain fields for the second and the
nanosecond parts."

**Repo:** `crates/dcps/src/time.rs` (`Time`, `Duration`: `sec`, `nanosec`)

**Tests:** `crates/dcps/src/time.rs::tests::time_seconds_and_nanoseconds_accessors`,
`crates/dcps/src/time.rs::tests::duration_seconds_and_nanoseconds_accessors`,
`crates/dcps/src/time.rs::tests::a_qos_duration_converts_to_sec_and_nanosec_and_back`

**Status:** done — die zugehörigen Einzelpunkte sind erledigt; siehe ihre Tests.

### §2.3.2 Dauern der QoS-Policies als Duration_t

**Spec:** §2.3.3, S. 157–159 — `LifespanQosPolicy { Duration_t duration; }`,
`DeadlineQosPolicy { Duration_t period; }` und weitere; `struct Duration_t {
long sec; unsigned long nanosec; }` (S. 151).

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::a_qos_duration_converts_to_sec_and_nanosec_and_back`

**Status:** done — die QoS-Dauern behalten die RTPS-Form (`seconds`, `fraction`) und wandeln sich per `From` in die `Duration` mit `sec`/`nanosec` und zurück; die unendliche bleibt unendlich.

### §2.3.2 normalisierte Darstellung 0 <= nanosec < 1000000000

**Spec:** §2.3.2, S. 150 — "These types are further constrained to always use a
‘normalized’ representation for the time, that is, the nanosec field must verify
0 <= nanosec < 1000000000."

**Repo:** `crates/dcps/src/time.rs` (`Time::new`, `Duration::new`, `is_normalized`, `from_millis`);
`crates/dcps/src/publisher.rs` (`check_timestamp`);
`crates/zerodds-c-api/src/extra_ffi.rs` (`*_w_timestamp`);
`crates/qos/src/duration.rs`

**Tests:** `crates/dcps/src/time.rs::tests::the_constructors_keep_nanosec_below_a_second`,
`crates/dcps/src/time.rs::tests::from_millis_of_a_negative_value_is_normalized`,
`crates/dcps/tests/sample_info_lifecycle.rs::a_timestamp_out_of_range_is_bad_parameter`,
`crates/zerodds-c-api/src/extra_ffi.rs::tests::dw_w_timestamp_calls_refuse_a_nanosec_out_of_range`

**Status:** `done` — `Time::new` und `Duration::new` übertragen volle Sekunden von `nanosec` nach `sec` (die Werte `TIME_INVALID`, `TIME_INFINITE`, `DURATION_INFINITE` bleiben); `from_millis` rundet nach unten, auch bei negativen Werten. Von Hand über die öffentlichen Felder gebaute Werte meldet `is_normalized`; die vier `*_w_timestamp` lehnen `nanosec` ab 10^9 mit BAD_PARAMETER ab, auch im C-API. Dauern der QoS-Policies sind in Bruchteilen von 2^-32 s gespeichert und damit normalisiert.

### §2.3.2 Native Typen

**Spec:** §2.3.2, S. 150 — "The IDL PSM introduces a number of types that are
intended to be defined in a native way … The type used in the IDL by this means
is not normative and an implementation is allowed to use any other type,
including non-scalar (i.e., structured types)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; `DomainId_t` ist `i32`,
`InstanceHandle_t` ist `InstanceHandle` (`u64`), `BuiltinTopicKey_t` ist der
16-Byte-GUID.

### §2.3.3 HANDLE_NIL

**Spec:** §2.3.3, S. 150–151 — "#define HANDLE_NIL_NATIVE 0 … const
InstanceHandle_t HANDLE_NIL = HANDLE_NIL_NATIVE;"

**Repo:** `crates/dcps/src/instance_handle.rs` (`HANDLE_NIL`)

**Tests:** `crates/dcps/src/instance_handle.rs::tests::handle_nil_is_zero`

**Status:** done

### §2.3.3 LENGTH_UNLIMITED

**Spec:** §2.3.3, S. 151 — "const long LENGTH_UNLIMITED = -1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`LENGTH_UNLIMITED`);
`crates/qos/src/policies/resource_limits.rs` (`LENGTH_UNLIMITED`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::length_unlimited_is_minus_one`,
`crates/qos/src/policies/resource_limits.rs::tests::encode_decode_negative_one`

**Status:** done

### §2.3.3 DURATION_INFINITE

**Spec:** §2.3.3, S. 151 — "const long DURATION_INFINITE_SEC = 0x7fffffff; const
unsigned long DURATION_INFINITE_NSEC = 0x7fffffff;"

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_values_are_those_of_the_idl`

**Status:** done — `Duration::INFINITE` ist `{ sec: 0x7fffffff, nanosec: 0x7fffffff }` wie in der IDL (nanosec war `0xffffffff`).

### §2.3.3 DURATION_ZERO

**Spec:** §2.3.3, S. 151 — "const long DURATION_ZERO_SEC = 0; const unsigned
long DURATION_ZERO_NSEC = 0;"

**Repo:** `crates/dcps/src/time.rs` (`Duration::ZERO`);
`crates/qos/src/duration.rs` (`Duration::ZERO`)

**Tests:** `crates/dcps/src/time.rs::tests::duration_zero_and_infinite_sentinels`,
`crates/qos/src/duration.rs::tests::zero_is_default_and_zero`

**Status:** done

### §2.3.3 TIME_INVALID

**Spec:** §2.3.3, S. 151 — "const long TIME_INVALID_SEC = -1; const unsigned
long TIME_INVALID_NSEC = 0xffffffff;"

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_values_are_those_of_the_idl`

**Status:** done — der Test deckt es ab.

### §2.3.3 Rückgabecodes RETCODE_OK bis RETCODE_ILLEGAL_OPERATION

**Spec:** §2.3.3, S. 151 — "const ReturnCode_t RETCODE_OK = 0; … RETCODE_ERROR =
1; … RETCODE_ILLEGAL_OPERATION = 12;"

**Repo:** `crates/dcps/src/error.rs` (`return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`

**Status:** done — zusätzlich `NOT_ALLOWED_BY_SECURITY = 13` aus DDS-Security.

### §2.3.3 StatusKind-Bits

**Spec:** §2.3.3, S. 151–152 — "const StatusKind INCONSISTENT_TOPIC_STATUS =
0x0001 << 0; … SUBSCRIPTION_MATCHED_STATUS = 0x0001 << 14;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`

**Status:** done — die Werte stimmen mit der IDL überein (Bits 0, 1, 2, 5 bis
14); die Namen haben kein Suffix `_STATUS`.

### §2.3.3 Status-Strukturen

**Spec:** §2.3.3, S. 152–153 — `InconsistentTopicStatus`, `SampleLostStatus`,
`SampleRejectedStatus`, `LivelinessLostStatus`, `LivelinessChangedStatus`,
`OfferedDeadlineMissedStatus`, `RequestedDeadlineMissedStatus`,
`QosPolicyCount`, `OfferedIncompatibleQosStatus`,
`RequestedIncompatibleQosStatus`, `PublicationMatchedStatus`,
`SubscriptionMatchedStatus`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — dieselben Strukturen wie im PIM; ihre Felder
sind unter §2.2.4.1 einzeln geführt.

### §2.3.3 SampleRejectedStatusKind

**Spec:** §2.3.3, S. 152 — "enum SampleRejectedStatusKind { NOT_REJECTED,
REJECTED_BY_INSTANCES_LIMIT, REJECTED_BY_SAMPLES_LIMIT,
REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT };"

**Repo:** `crates/dcps/src/status.rs` (`SampleRejectedStatusKind`)

**Tests:** `crates/dcps/src/status.rs::tests::sample_rejected_kind_variants_are_distinct`,
`crates/dcps/src/status.rs::tests::sample_rejected_status_default_kind_is_not_rejected`

**Status:** done — vier Varianten in der Reihenfolge der IDL.

### §2.3.3 Listener-Schnittstellen

**Spec:** §2.3.3, S. 154–155 — `TopicListener`, `DataWriterListener`,
`PublisherListener : DataWriterListener`, `DataReaderListener`,
`SubscriberListener : DataReaderListener` mit `on_data_on_readers(in Subscriber
the_subscriber)`, `DomainParticipantListener : TopicListener, PublisherListener,
SubscriberListener`; jede Operation mit `in DataWriter writer`, `in DataReader
the_reader` oder `in Topic the_topic`.

**Repo:** —

**Tests:** `crates/dcps/tests/listener_entity.rs::a_shared_listener_is_called_with_the_entity`

**Status:** done — die Vererbung und die Operationen entsprechen der IDL (`crates/dcps/src/listener.rs`); der erste Parameter der Entity-Form ist die Entity (siehe §2.2.4.3).

### §2.3.3 Condition, WaitSet, GuardCondition, StatusCondition

**Spec:** §2.3.3, S. 155 — `Condition::get_trigger_value`, `WaitSet::wait(inout
ConditionSeq active_conditions, in Duration_t timeout)`, `attach_condition`,
`detach_condition`, `get_conditions`, `GuardCondition::set_trigger_value`,
`StatusCondition::get_enabled_statuses`, `set_enabled_statuses`, `get_entity`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — dieselben Operationen wie im PIM; sie sind
unter §2.2.2.1.6 bis §2.2.2.1.9 einzeln geführt.

### §2.3.3 SampleStateKind-Bits

**Spec:** §2.3.3, S. 155 — "const SampleStateKind READ_SAMPLE_STATE = 0x0001 <<
0; const SampleStateKind NOT_READ_SAMPLE_STATE = 0x0001 << 1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state::READ_SAMPLE_STATE`,
`state::NOT_READ_SAMPLE_STATE`),
`crates/dcps/src/sample_info.rs::sample_state_mask`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — Werte der IDL in beiden Modulen.

### §2.3.3 ViewStateKind-Bits

**Spec:** §2.3.3, S. 156 — "const ViewStateKind NEW_VIEW_STATE = 0x0001 << 0;
const ViewStateKind NOT_NEW_VIEW_STATE = 0x0001 << 1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state::NEW_VIEW_STATE`,
`state::NOT_NEW_VIEW_STATE`), `crates/dcps/src/sample_info.rs::view_state_mask`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — Werte der IDL in beiden Modulen.

### §2.3.3 InstanceStateKind-Bits und NOT_ALIVE_INSTANCE_STATE

**Spec:** §2.3.3, S. 156 — "const InstanceStateKind ALIVE_INSTANCE_STATE =
0x0001 << 0; … NOT_ALIVE_DISPOSED_INSTANCE_STATE = 0x0001 << 1; …
NOT_ALIVE_NO_WRITERS_INSTANCE_STATE = 0x0001 << 2; … const InstanceStateMask
NOT_ALIVE_INSTANCE_STATE = 0x006;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state`);
`crates/dcps/src/sample_info.rs` (`instance_state_mask`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::instance_state_not_alive_is_or_of_disposed_and_no_writers`

**Status:** done

### §2.3.3 ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE = 0xffff

**Spec:** §2.3.3, S. 155–156 — "const SampleStateMask ANY_SAMPLE_STATE = 0xffff;
… const ViewStateMask ANY_VIEW_STATE = 0xffff; … const InstanceStateMask
ANY_INSTANCE_STATE = 0xffff;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state`),
`crates/dcps/src/sample_info.rs`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — die drei Masken sind 0xffff.

### §2.3.3 ReadCondition und QueryCondition

**Spec:** §2.3.3, S. 156 — `ReadCondition::get_sample_state_mask`,
`get_view_state_mask`, `get_instance_state_mask`, `get_datareader`;
`QueryCondition::get_query_expression`, `get_query_parameters`,
`set_query_parameters`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — dieselben Operationen wie im PIM; sie sind
unter §2.2.2.5.8 und §2.2.2.5.9 einzeln geführt.

### §2.3.3 Namen der QoS-Policies

**Spec:** §2.3.3, S. 156–157 — "const string USERDATA_QOS_POLICY_NAME =
"UserData"; … const string DURABILITYSERVICE_POLICY_NAME = "DurabilityService";"

**Repo:** `crates/qos/src/policies/name.rs`, `crates/dcps/src/psm_constants.rs` (`qos_policy_name`)

**Tests:** `crates/qos/src/policies/name.rs::tests::every_policy_has_its_name`

**Status:** done — die Namenskonstanten der 22 Policies stehen in `zerodds_qos::policy_name` und `zerodds_dcps::psm_constants::qos_policy_name`; die Policies haben `QosPolicy::name` (§2.2.2.1.3). Zu GROUPDATA siehe dort.

### §2.3.3 QosPolicyId_t-Werte

**Spec:** §2.3.3, S. 157 — "const QosPolicyId_t INVALID_QOS_POLICY_ID = 0; const
QosPolicyId_t USERDATA_QOS_POLICY_ID = 1; … DURABILITYSERVICE_QOS_POLICY_ID =
22;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`qos_policy_id`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::qos_policy_ids_are_distinct`

**Status:** done — die Werte 0 bis 22 stimmen mit der IDL überein; die Namen
lauten `qos_policy_id::USER_DATA` statt `USERDATA_QOS_POLICY_ID`. Zusätzlich 23
und 24 aus XTypes 1.3.

### §2.3.3 UserDataQosPolicy

**Spec:** §2.3.3, S. 157 — `struct UserDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::user_data_roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 TopicDataQosPolicy

**Spec:** §2.3.3, S. 157 — `struct TopicDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::empty_data_roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 GroupDataQosPolicy

**Spec:** §2.3.3, S. 157 — `struct GroupDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::group_data_large_payload`

**Status:** done — Felder wie in der IDL.

### §2.3.3 TransportPriorityQosPolicy

**Spec:** §2.3.3, S. 157 — `struct TransportPriorityQosPolicy`: long value.

**Repo:** `crates/qos/src/policies/transport_priority.rs`
(`TransportPriorityQosPolicy`)

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 LifespanQosPolicy

**Spec:** §2.3.3, S. 157 — `struct LifespanQosPolicy`: Duration_t duration.

**Repo:** `crates/qos/src/policies/lifespan.rs` (`LifespanQosPolicy`)

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 DurabilityQosPolicy

**Spec:** §2.3.3, S. 157 — `struct DurabilityQosPolicy`: DurabilityQosPolicyKind
kind (VOLATILE_DURABILITY_QOS, TRANSIENT_LOCAL_DURABILITY_QOS,
TRANSIENT_DURABILITY_QOS, PERSISTENT_DURABILITY_QOS).

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/durability.rs::tests::ordering_matches_spec`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten.

### §2.3.3 PresentationQosPolicy

**Spec:** §2.3.3, S. 158 — `struct PresentationQosPolicy`:
PresentationQosPolicyAccessScopeKind access_scope (INSTANCE_PRESENTATION_QOS,
TOPIC_PRESENTATION_QOS, GROUP_PRESENTATION_QOS); boolean coherent_access;
boolean ordered_access.

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/qos/src/policies/presentation.rs::tests::scope_ordering`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten.

### §2.3.3 DeadlineQosPolicy

**Spec:** §2.3.3, S. 158 — `struct DeadlineQosPolicy`: Duration_t period.

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 LatencyBudgetQosPolicy

**Spec:** §2.3.3, S. 158 — `struct LatencyBudgetQosPolicy`: Duration_t duration.

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 OwnershipQosPolicy

**Spec:** §2.3.3, S. 158 — `struct OwnershipQosPolicy`: OwnershipQosPolicyKind
kind (SHARED_OWNERSHIP_QOS, EXCLUSIVE_OWNERSHIP_QOS).

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::roundtrip`,
`crates/qos/src/policies/ownership.rs::tests::try_from_u32_valid_variants`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten.

### §2.3.3 OwnershipStrengthQosPolicy

**Spec:** §2.3.3, S. 158 — `struct OwnershipStrengthQosPolicy`: long value.

**Repo:** `crates/qos/src/policies/ownership_strength.rs`
(`OwnershipStrengthQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_negative`

**Status:** done — Felder wie in der IDL.

### §2.3.3 LivelinessQosPolicy

**Spec:** §2.3.3, S. 158 — `struct LivelinessQosPolicy`: LivelinessQosPolicyKind
kind (AUTOMATIC_LIVELINESS_QOS, MANUAL_BY_PARTICIPANT_LIVELINESS_QOS,
MANUAL_BY_TOPIC_LIVELINESS_QOS); Duration_t lease_duration.

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessQosPolicy`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::roundtrip`,
`crates/qos/src/policies/liveliness.rs::tests::kind_ordering`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 TimeBasedFilterQosPolicy

**Spec:** §2.3.3, S. 158 — `struct TimeBasedFilterQosPolicy`: Duration_t
minimum_separation.

**Repo:** `crates/qos/src/policies/time_based_filter.rs`
(`TimeBasedFilterQosPolicy`)

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 PartitionQosPolicy

**Spec:** §2.3.3, S. 158 — `struct PartitionQosPolicy`: StringSeq name.

**Repo:** `crates/qos/src/policies/partition.rs` (`PartitionQosPolicy`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::roundtrip_two_names`

**Status:** done — das Feld heißt `names` statt `name`.

### §2.3.3 ReliabilityQosPolicy

**Spec:** §2.3.3, S. 158 — `struct ReliabilityQosPolicy`:
ReliabilityQosPolicyKind kind (BEST_EFFORT_RELIABILITY_QOS,
RELIABLE_RELIABILITY_QOS); Duration_t max_blocking_time.

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/reliability.rs::tests::ordering_reliable_higher_than_besteffort`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten. Die Diskriminanten von
`ReliabilityKind` sind die RTPS-Werte 1 und 2, nicht die Ordinalzahlen 0 und 1
der IDL.

### §2.3.3 DestinationOrderQosPolicy

**Spec:** §2.3.3, S. 159 — `struct DestinationOrderQosPolicy`:
DestinationOrderQosPolicyKind kind (BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS,
BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS).

**Repo:** `crates/qos/src/policies/destination_order.rs`
(`DestinationOrderQosPolicy`)

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::roundtrip`,
`crates/qos/src/policies/destination_order.rs::tests::kind_ordering`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten.

### §2.3.3 HistoryQosPolicy

**Spec:** §2.3.3, S. 159 — `struct HistoryQosPolicy`: HistoryQosPolicyKind kind
(KEEP_LAST_HISTORY_QOS, KEEP_ALL_HISTORY_QOS); long depth.

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryQosPolicy`)

**Tests:** `crates/qos/src/policies/history.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/history.rs::tests::encode_decode_keep_all`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten.

### §2.3.3 ResourceLimitsQosPolicy

**Spec:** §2.3.3, S. 159 — `struct ResourceLimitsQosPolicy`: long max_samples;
long max_instances; long max_samples_per_instance.

**Repo:** `crates/qos/src/policies/resource_limits.rs`
(`ResourceLimitsQosPolicy`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::encode_decode_roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 EntityFactoryQosPolicy

**Spec:** §2.3.3, S. 159 — `struct EntityFactoryQosPolicy`: boolean
autoenable_created_entities.

**Repo:** `crates/qos/src/policies/entity_factory.rs` (`EntityFactoryQosPolicy`)

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 WriterDataLifecycleQosPolicy

**Spec:** §2.3.3, S. 159 — `struct WriterDataLifecycleQosPolicy`: boolean
autodispose_unregistered_instances.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`WriterDataLifecycleQosPolicy`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_roundtrip`

**Status:** done — Felder wie in der IDL.

### §2.3.3 ReaderDataLifecycleQosPolicy

**Spec:** §2.3.3, S. 159 — `struct ReaderDataLifecycleQosPolicy`: Duration_t
autopurge_nowriter_samples_delay; Duration_t autopurge_disposed_samples_delay.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`ReaderDataLifecycleQosPolicy`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_roundtrip`

**Status:** done — Felder wie in der IDL. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 DurabilityServiceQosPolicy

**Spec:** §2.3.3, S. 159 — `struct DurabilityServiceQosPolicy`: Duration_t
service_cleanup_delay; HistoryQosPolicyKind history_kind; long history_depth;
long max_samples; long max_instances; long max_samples_per_instance.

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::roundtrip`

**Status:** done — Felder wie in der IDL; die Art ist ein Rust-`enum`, ohne das
Suffix `_QOS` in den Namen der Varianten. Die Dauern sind
`zerodds_qos::Duration` (Item „Dauern der QoS-Policies als Duration_t“).

### §2.3.3 DomainParticipantFactoryQos

**Spec:** §2.3.3, S. 159–160 — `struct DomainParticipantFactoryQos`:
entity_factory.

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactoryQos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`

**Status:** done — Felder wie in der IDL.

### §2.3.3 DomainParticipantQos

**Spec:** §2.3.3, S. 160 — `struct DomainParticipantQos`: user_data,
entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`DomainParticipantQos`)

**Tests:** `crates/dcps/tests/participant_config_user_data.rs::an_explicit_config_keeps_the_user_data_of_the_qos`

**Status:** done — Felder wie in der IDL.

### §2.3.3 TopicQos

**Spec:** §2.3.3, S. 160 — `struct TopicQos`: topic_data, durability,
durability_service, deadline, latency_budget, liveliness, reliability,
destination_order, history, resource_limits, transport_priority, lifespan,
ownership.

**Repo:** `crates/dcps/src/qos.rs` (`TopicQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`

**Status:** done — Felder wie in der IDL.

### §2.3.3 DataWriterQos

**Spec:** §2.3.3, S. 160 — `struct DataWriterQos`: durability,
durability_service, deadline, latency_budget, liveliness, reliability,
destination_order, history, resource_limits, transport_priority, lifespan,
user_data, ownership, ownership_strength, writer_data_lifecycle.

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos`)

**Tests:** `crates/dcps/src/publisher.rs::tests::create_datawriter_wires_history_keep_last_depth`

**Status:** done — die Felder der IDL und zusätzlich `partition`,
`presentation`, `topic_data`, `group_data` als Überschreibung je Endpunkt sowie
`data_representation` aus XTypes 1.3.

### §2.3.3 PublisherQos

**Spec:** §2.3.3, S. 160 — `struct PublisherQos`: presentation, partition,
group_data, entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`PublisherQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done — Felder wie in der IDL.

### §2.3.3 DataReaderQos

**Spec:** §2.3.3, S. 160–161 — `struct DataReaderQos`: durability, deadline,
latency_budget, liveliness, reliability, destination_order, history,
resource_limits, user_data, ownership, time_based_filter, reader_data_lifecycle.

**Repo:** `crates/dcps/src/qos.rs` (`DataReaderQos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done — die Felder der IDL und zusätzlich `partition`,
`presentation`, `topic_data`, `group_data` als Überschreibung je Endpunkt sowie
`data_representation` und `type_consistency` aus XTypes 1.3.

### §2.3.3 SubscriberQos

**Spec:** §2.3.3, S. 161 — `struct SubscriberQos`: presentation, partition,
group_data, entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`SubscriberQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`

**Status:** done — Felder wie in der IDL.

### §2.3.3 Strukturen der Built-in-Topics

**Spec:** §2.3.3, S. 161–162 — `ParticipantBuiltinTopicData`,
`TopicBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — dieselben Felder wie die Tabelle in §2.2.5;
sie sind dort einzeln geführt.

### §2.3.3 Entity

**Spec:** §2.3.3, S. 162 — `interface Entity` mit den Operationen des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.1.1 einzeln geführt.

### §2.3.3 DomainParticipant

**Spec:** §2.3.3, S. 163–164 — `interface DomainParticipant` mit den Operationen
des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.2.1 einzeln geführt.

### §2.3.3 DomainParticipantFactory

**Spec:** §2.3.3, S. 165 — `interface DomainParticipantFactory` mit den
Operationen des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.2.2 einzeln geführt.

### §2.3.3 TopicDescription, Topic, ContentFilteredTopic, MultiTopic

**Spec:** §2.3.3, S. 165–166 — `interface TopicDescription, Topic,
ContentFilteredTopic, MultiTopic` mit den Operationen des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.3 einzeln geführt.

### §2.3.3 Publisher

**Spec:** §2.3.3, S. 166 — `interface Publisher` mit den Operationen des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.4.1 einzeln geführt.

### §2.3.3 DataWriter

**Spec:** §2.3.3, S. 167–168 — `interface DataWriter` mit den Operationen des
PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.4.2 einzeln geführt.

### §2.3.3 Subscriber

**Spec:** §2.3.3, S. 168–169 — `interface Subscriber` mit den Operationen des
PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.5.2 einzeln geführt.

### §2.3.3 DataReader

**Spec:** §2.3.3, S. 169–171 — `interface DataReader` mit den Operationen des
PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.5.3 einzeln geführt.

### §2.3.3 SampleInfo

**Spec:** §2.3.3, S. 171–172 — `interface SampleInfo` bzw. `struct SampleInfo`
mit den Operationen des PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiederholung des PIM in IDL; die Operationen
und Felder sind unter §2.2.2.5.5 einzeln geführt.

### §2.3.3 TypeSupport und FooTypeSupport

**Spec:** §2.3.3, S. 165, 172 — "interface TypeSupport { // ReturnCode_t
register_type(…); // string get_type_name(); };" und "interface FooTypeSupport :
DDS::TypeSupport { DDS::ReturnCode_t register_type(…); string get_type_name();
};"

**Repo:** `crates/dcps/src/type_support.rs`

**Tests:** `crates/dcps/tests/type_support.rs::the_type_support_names_its_type`, `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`

**Status:** done — `TypeSupport<T>` mit `register_type` und `get_type_name`; `FooTypeSupport` ist `TypeSupport<Foo>`.

### §2.3.3 FooDataWriter und FooDataReader

**Spec:** §2.3.3, S. 172–174 — "Implied IDL for type "Foo"": `interface
FooDataWriter : DDS::DataWriter` und `interface FooDataReader : DDS::DataReader`
mit den typisierten Operationen.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`);
`crates/dcps/src/subscriber.rs` (`DataReader<T>`); `crates/dcps/src/dds_type.rs`
(`DdsType`)

**Tests:** `crates/zerodds-build/tests/compile.rs::a_struct_becomes_a_type_with_a_codec`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`

**Status:** done — die typisierten Schnittstellen sind die Generics
`DataWriter<T>` und `DataReader<T>`; die einzelnen Operationen sind unter
§2.2.2.4.2 und §2.2.2.5.3 geführt.

### §2.3.3 Platzhalter TheParticipantFactory und *_QOS_DEFAULT

**Spec:** §2.3.3, S. 150 — "#define TheParticipantFactory #define
PARTICIPANT_QOS_DEFAULT … #define DATAWRITER_QOS_USE_TOPIC_QOS #define
DATAREADER_QOS_USE_TOPIC_QOS".

**Repo:** `crates/dcps/src/psm_constants.rs` (`PARTICIPANT_QOS_DEFAULT` …
`DATAREADER_QOS_USE_TOPIC_QOS`),
`crates/dcps/src/factory.rs::THE_PARTICIPANT_FACTORY`

**Tests:**
`crates/dcps/tests/default_qos.rs::the_participant_factory_is_the_instance`,
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`,
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — die acht Platzhalter als Werte von `QosArg` und
`EndpointQosArg`, `TheParticipantFactory` als `THE_PARTICIPANT_FACTORY`.

## Annex A Compliance Points

### Annex A Minimum profile

**Spec:** Annex A, S. 175 — "Minimum profile: This profile contains just the
mandatory features of DCPS. None of the optional features are included."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Profildefinition; die Pflichtteile sind oben
Item für Item geführt.

### Annex A Content-subscription profile

**Spec:** Annex A, S. 175 — "Content-subscription profile: This profile adds the
optional classes: ContentFilteredTopic, QueryCondition, MultiTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic`, `MultiTopic`);
`crates/dcps/src/condition.rs` (`QueryCondition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/src/topic.rs::tests::cft_param_index_out_of_range_rejected`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`, `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`

**Status:** done — ContentFilteredTopic, QueryCondition und MultiTopic (SELECT, FROM, NATURAL JOIN, WHERE) sind umgesetzt.

### Annex A Persistence profile

**Spec:** Annex A, S. 175 — "Persistence profile: This profile adds the optional
Qos policy DURABILITY_SERVICE as well as the optional settings ‘TRANSIENT’ and
‘PERSISTENT’ of the DURABILITY QoS policy kind."

**Repo:** `crates/dcps/src/durability_service.rs`;
`crates/qos/src/policies/durability_service.rs`

**Tests:** `crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::persistent_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`

**Status:** done — TRANSIENT und PERSISTENT liefern an späte Reader, mit den fiktiven Entities des Dienstes, `service_cleanup_delay` und LIFESPAN im Speicher (§2.2.3.4, §2.2.3.16).

### Annex A Ownership profile

**Spec:** Annex A, S. 175 — "Ownership profile: This profile adds two things
First the optional setting ‘EXCLUSIVE’ of the OWNERSHIP kind. Second support for
the optional OWNERSHIP_STRENGTH policy. Third the ability to set a depth > 1 for
the HISTORY QoS policy."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`should_accept_sample_under_exclusive_ownership`);
`crates/dcps/src/subscriber.rs` (`DataReader::evict_oldest_of_instance`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_n_keeps_the_newest_n_per_instance`,
`crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** done — EXCLUSIVE, OWNERSHIP_STRENGTH und HISTORY depth > 1 sind umgesetzt, mit dem Wechsel des Eigentums bei Verlust der Liveliness, DEADLINE, Abmeldung und Löschen (§2.2.3.9.2).

### Annex A Group access profile

**Spec:** Annex A, S. 175 — "Group access profile: This profile includes support
for the PRESENTATION access_scope setting of ‘GROUP’".

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/coherent_set.rs`

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — `begin_access`, `get_datareaders` in GSN-Reihenfolge, `end_access` (§2.2.2.5.2.10).

## Annex B Syntax for Queries and Filters

### B.1 Einsatz der SQL-Teilmenge

**Spec:** Annex B.1, S. 177 — "A subset of SQL syntax is used in several parts
of the specification: The filter_expression in the ContentFilteredTopic … The
topic_expression in the MultiTopic … The query_expression in the
QueryReadCondition".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Grammatik ist in den folgenden
Items geführt.

### B.2 FilterExpression ::= Condition

**Spec:** Annex B.2, S. 177 — "FilterExpression ::= Condition".

**Repo:** `crates/sql-filter/src/parser.rs` (`parse`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_simple_eq`,
`crates/sql-filter/src/parser.rs::tests::parse_rejects_trailing_garbage`

**Status:** done

### B.2 TopicExpression ::= SelectFrom {Where} ‘;’

**Spec:** Annex B.2, S. 177 — "TopicExpression ::= SelectFrom {Where } ‘;’".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::the_annex_b_example_parses`,
`crates/sql-filter/src/topic_expression.rs::tests::anything_else_is_refused`

**Status:** `done` — `zerodds_sql_filter::parse_topic_expression`; das schließende `;` ist wie im Beispiel von Annex B.2 optional; alles andere ist ein `ParseError`.

### B.2 SelectFrom und Aggregation

**Spec:** Annex B.2, S. 177–178 — "SelectFrom ::= ‘SELECT’ Aggregation ‘FROM’
Selection . Aggregation ::= ‘*’ | (SubjectFieldSpec // ‘,’)".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::star_one_topic_no_where_and_the_closing_semicolon`

**Status:** `done` — `*` und die Liste der Felder.

### B.2 SubjectFieldSpec mit AS

**Spec:** Annex B.2, S. 177–178 — "SubjectFieldSpec ::= FIELDNAME | FIELDNAME
‘AS’ FIELDNAME | FIELDNAME FIELDNAME".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_field_takes_an_alias_with_as_or_juxtaposed`

**Status:** `done` — `FIELDNAME`, `FIELDNAME AS FIELDNAME` und `FIELDNAME FIELDNAME`; zwei Felder gleichen Ergebnisnamens sind ein Fehler.

### B.2 Selection und JoinItem

**Spec:** Annex B.2, S. 178 — "Selection ::= TOPICNAME | TOPICTNAME NaturalJoin
JoinItem . JoinItem ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem | ‘(’
TOPICNAME NaturalJoin JoinItem ‘)’".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::join_items_may_nest_in_parentheses`,
`crates/sql-filter/src/topic_expression.rs::tests::join_items_nest_to_a_bound`

**Status:** `done` — auch geklammerte JoinItems (Tiefe bis 64); ein geklammertes Item ohne Join und ein Topic zweimal sind Fehler.

### B.2 NaturalJoin und ihre Aliase

**Spec:** Annex B.2, S. 178 — "NaturalJoin ::= ‘INNER NATURAL JOIN’ | ‘NATURAL
JOIN’ | ‘NATURAL INNER JOIN’ … NOTE: INNER NATURAL JOIN, NATURAL JOIN, and
NATURAL INNER JOIN are all aliases".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::the_three_spellings_of_the_natural_join`

**Status:** `done` — `INNER NATURAL JOIN`, `NATURAL JOIN`, `NATURAL INNER JOIN`, ohne Rücksicht auf Groß- und Kleinschreibung.

### B.2 Where

**Spec:** Annex B.2, S. 178 — "Where ::= ‘WHERE’ Condition".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::where_in_a_string_is_not_the_keyword`

**Status:** `done` — der Rest nach `WHERE` ist eine Condition; `WHERE` in einem String ist kein Schlüsselwort.

### B.2 TOPICNAME

**Spec:** Annex B.2, S. 179 — "TOPICNAME - A topic name is an identifier for a
topic, and is defined as any series of characters ‘a’, ..., ‘z’, ‘A’, ..., ‘Z’,
‘0’, ..., ‘9’, ‘-’ but may not start with a digit.".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_topic_name_is_letters_digits_and_hyphen_or_quoted`

**Status:** `done` — Buchstaben, Ziffern und `-`, nicht mit einer Ziffer beginnend; jeder andere Name in Hochkommas (wie im Beispiel von Annex B.2).

### B.2 QueryExpression — Condition

**Spec:** Annex B.2, S. 177 — "QueryExpression ::= {Condition}{‘ORDER BY’
(FIELDNAME // ‘,’) }".

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition`:
`zerodds_sql_filter::parse`)

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_query_expression_is_a_condition_and_an_order`

**Status:** `done` — `parse_query_expression`.

### B.2 QueryExpression — ORDER BY

**Spec:** Annex B.2, S. 177 — "{‘ORDER BY’ (FIELDNAME // ‘,’) }".

**Repo:** —

**Tests:** `crates/dcps/tests/query_condition.rs::order_by_orders_what_a_read_or_take_returns`,
`crates/zerodds-c-api/tests/query_condition_read.rs::order_by_orders_what_read_and_take_return`

**Status:** `done` — `ORDER BY` mit einem oder mehreren FIELDNAMEs; die Samples, die ein read oder take mit der QueryCondition liefert, sind danach aufsteigend geordnet (Rust und C-API). Die Spec nennt keine Richtung; aufsteigend ist der SQL-Default. Im C-API gilt `max_samples` für die sortierte Auswahl.

### B.2 Condition

**Spec:** Annex B.2, S. 178 — "Condition ::= Predicate | Condition ‘AND’
Condition | Condition ‘OR’ Condition | ‘NOT’ Condition | ‘(’ Condition ‘)’".

**Repo:** `crates/sql-filter/src/parser.rs` (`parse_or`, `parse_and`,
`parse_not`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_and_or_precedence`,
`crates/sql-filter/src/parser.rs::tests::parse_not`,
`crates/sql-filter/src/parser.rs::tests::parse_parenthesized`,
`crates/sql-filter/src/evaluator.rs::tests::evaluates_boolean_not_or`

**Status:** done

### B.2 Predicate

**Spec:** Annex B.2, S. 178 — "Predicate ::= ComparisonPredicate |
BetweenPredicate".

**Repo:** `crates/sql-filter/src/parser.rs`

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_simple_eq`,
`crates/sql-filter/src/parser.rs::tests::parse_between_predicate`

**Status:** done

### B.2 ComparisonPredicate

**Spec:** Annex B.2, S. 178 — "ComparisonPredicate ::= FIELDNAME RelOp Parameter
| Parameter RelOp FIELDNAME | FIELDNAME RelOp FIELDNAME".

**Repo:** `crates/sql-filter/src/parser.rs` (`cmp = operand cmp_op operand`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_param`,
`crates/sql-filter/src/evaluator.rs::tests::evaluates_int_compare`

**Status:** done — der Parser nimmt jede Kombination aus Feld, Literal und
Parameter an, auch zwei Literale.

### B.2 BetweenPredicate

**Spec:** Annex B.2, S. 178 — "BetweenPredicate ::= FIELDNAME ‘BETWEEN’ Range |
FIELDNAME ‘NOT BETWEEN’ Range".

**Repo:** `crates/sql-filter/src/parser.rs` (`Expr::Between`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_between_predicate`,
`crates/sql-filter/src/parser.rs::tests::parse_not_between_predicate`

**Status:** done

### B.2 RelOp

**Spec:** Annex B.2, S. 178 — "RelOp ::= ‘=’ | ‘>’ | ‘>=’ | ‘<’ | ‘<=’ | ‘<>’ |
like".

**Repo:** `crates/sql-filter/src/lexer.rs`; `crates/sql-filter/src/evaluator.rs`
(`like_match`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::lex_mixed_operators`,
`crates/sql-filter/src/evaluator.rs::tests::like_wildcards`

**Status:** done — zusätzlich `!=` als Synonym für `<>`.

### B.2 Range

**Spec:** Annex B.2, S. 178 — "Range ::= Parameter ‘AND’ Parameter".

**Repo:** `crates/sql-filter/src/parser.rs`

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_between_with_params`

**Status:** done

### B.2 Parameter

**Spec:** Annex B.2, S. 179 — "Parameter ::= INTEGERVALUE | CHARVALUE |
FLOATVALUE | STRING | ENUMERATEDVALUE | PARAMETER".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Token sind einzeln unten
geführt.

### B.2 FIELDNAME

**Spec:** Annex B.2, S. 179 — "FIELDNAME - A fieldname is a reference to a field
in the data-structure. The dot ‘.’ is used to navigate through nested structures
… The names of the field are those specified in the IDL definition of the
corresponding structure".

**Repo:** `crates/sql-filter/src/lexer.rs` (Bezeichner mit `.`)

**Tests:** `crates/zerodds-c-api/src/subscriber_ffi.rs::tests::cft_type_with_a_dependency_filters_a_nested_member`,
`crates/sql-filter/src/evaluator.rs::tests::unknown_field_is_error`

**Status:** done

### B.2 INTEGERVALUE — Dezimalzahl mit Vorzeichen

**Spec:** Annex B.2, S. 179 — "INTEGERVALUE - Any series of digits, optionally
preceded by a plus or minus sign, representing a decimal integer value within
the range of the system."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::IntLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`

**Status:** `done` — `-` und `+`.

### B.2 INTEGERVALUE — Pluszeichen und 0x

**Spec:** Annex B.2, S. 179 — "optionally preceded by a plus or minus sign … A
hexadecimal number is preceded by 0x and must be a valid hexadecimal
expression."

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`,
`crates/sql-filter/src/lexer.rs::tests::a_malformed_hexadecimal_is_refused`

**Status:** `done` — ein `+` vor einer Zahl und `0x1F`; eine fehlerhafte Hexadezimalzahl ist ein Fehler.

### B.2 CHARVALUE

**Spec:** Annex B.2, S. 179 — "CHARVALUE - A single character enclosed between
single quotes."

**Repo:** —

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_filter_compares_an_enumerated_and_a_character_member`

**Status:** `done` — ein Zeichen in Hochkommas ist ein String aus einem Zeichen; ein Zeichenfeld eines Typs mit `TypeObject` liefert dasselbe.

### B.2 FLOATVALUE

**Spec:** Annex B.2, S. 179 — "FLOATVALUE - Any series of digits, optionally
preceded by a plus or minus sign and optionally including a floating point
(‘.’). A power-of-ten expression may be postfixed, which has the syntax en".

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::FloatLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`

**Status:** `done` — auch mit `+`.

### B.2 STRING

**Spec:** Annex B.2, S. 179 — "STRING - Any series of characters encapsulated in
single quotes, except a new-line character or a right quote."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::StrLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::lex_basic_comparison`,
`crates/sql-filter/src/lexer.rs::tests::lex_string_with_escape_quote`

**Status:** `done` — Zeichen in Hochkommas, zusätzlich `''` als Hochkomma (SQL-92); Zeichen außerhalb ASCII und der Zeilenumbruch siehe das folgende Item.

### B.2 STRING — Zeichen außerhalb ASCII und Zeilenumbruch

**Spec:** Annex B.2, S. 179 — "Any series of characters encapsulated in single
quotes, except a new-line character".

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::a_string_keeps_its_characters_and_refuses_a_new_line`

**Status:** `done` — die Zeichen werden als Zeichen gelesen, nicht als Bytes; ein Zeilenumbruch im String ist ein Fehler.

### B.2 ENUMERATEDVALUE

**Spec:** Annex B.2, S. 179 — "ENUMERATEDVALUE - An enumerated value is a
reference to a value declared within an enumeration. Enumerated values consist
of the name of the enumeration label enclosed in single quotes."

**Repo:** —

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_filter_compares_an_enumerated_and_a_character_member`

**Status:** `done` — ein Label in Hochkommas ist ein String; ein Aufzählungsfeld eines Typs mit `TypeObject` liefert den Namen seines Labels. Ein Typ, der `DdsType::field_value` selbst schreibt, liefert, was er liefert.

### B.2 PARAMETER %n, n < 100

**Spec:** Annex B.2, S. 179 — "PARAMETER - A parameter is of the form %n, where
n represents a natural number (zero included) smaller than 100. It refers to the
n + 1 th argument in the given context."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::Param`);
`crates/sql-filter/src/evaluator.rs`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::evaluates_param`,
`crates/sql-filter/src/lexer.rs::tests::lex_rejects_bad_parameter`

**Status:** done — `%0` ist das erste Argument; der Lexer nimmt auch Indizes ab
100 an.

### B.2 Beispiele

**Spec:** Annex B.2, S. 179–180 — "Example of a topic_expression: “SELECT
flight_name, x, y, z AS height FROM ‘Location’ NATURAL JOIN ‘FlightPlan’ WHERE
height < 1000 AND x <23” Example of a query_expression or a filter_expression:
“height < 1000 AND x <23”".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiele.

---

## Audit-Status

796 done / 0 partial / 0 open / 63 n/a (informative) / 0 n/a (rejected).

Test-Lauf: wird auf dem Linux-Testhost ergänzt.

