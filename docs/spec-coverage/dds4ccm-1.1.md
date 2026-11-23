# DDS for Lightweight CCM 1.1 — Spec-Coverage

**Spec:** [OMG DDS for Lightweight CCM 1.1 — formal/2012-02-01 (~95 Seiten) →](https://www.omg.org/spec/DDS4CCM/)

**Kontext:** DDS4CCM definiert wie CCM-Components mit DDS interagieren:
DDS-DCPS Extended Ports + Connectors (§7) für die Pub/Sub-Nutzung,
plus DDS-DLRL Extended Ports + Connectors (§8) für die optionale
Object-Layer-Nutzung. Die XML-QoS-Profile-Definition (§7.4.2 +
Annex C XSD + Annex D Default-Profile) ist die zentrale
QoS-Konfigurations-Spec.

Die Implementation ist über mehrere Crates verteilt:

- `crates/ccm/` — das Datenmodell der DDS-DCPS-Ports und der Connectoren: ihre Namen, die Basic Ports eines Port-Typs, die Mirror-Ports eines Connectors, die Ebenen der Threading-Policy (`src/dds4ccm.rs`)
- `crates/dds4ccm/` — die Connector-Fragmente der DDS-DCPS-Ports (§7.2.2), die Connectoren (§7.3), ihre DCPS-Entities und QoS-Profile (§7.4.1, §7.4.3) und die Threading-Policy (§7.4.4) über den DCPS-Entities; die IDL von Annex A (`idl/ccm_dds.idl`); die QoS-Profile der beiden Patterns (Annex E, `profiles/`); die DLRL-Ports und -Connectoren (§8, `src/dlrl.rs`) über den DLRL-Entities von `crates/dlrl/`; die IDL von Annex B (`idl/ccm_dds_dlrl.idl`)
- `crates/xml/` — der Leser der XML-QoS-Profile (§7.4.2), das Schema von Annex C (`schemas/dds4ccm_qos_profile.xsd`) und das Default-Profil von Annex D (`schemas/dds4ccm_default_qos_profile.xml`)
- `crates/dcps/` — die DCPS-Entities unter den Connector-Fragmenten
- `crates/idl-parse/` + `crates/idl-lower/` + `crates/idl-compose/` — lesen die IDL von Annex A (Template-Module, `porttype`, `connector`); `crates/ccm/` bildet Ports und Connectoren auf Equivalent-IDL ab
- `crates/corba-ccm/` — CCM-Container-Stack (siehe `omg-ccm-4.0.md`). Er hat keine Abhängigkeit auf `crates/ccm/` und `crates/dds4ccm/`: die Fragmente von §7 laufen im `Container` von `crates/dds4ccm/src/entities.rs`
- `crates/dlrl/` — hat `Cache`, `CacheAccess`, `CacheUsage`, `ObjectHome` und `CacheListener` von DDS-DLRL (`crates/dlrl/src/cache/`) und deren IDL (`crates/dlrl/idl/dds_dlrl.idl`); die DLRL-Ports und -Connectoren von §8 darauf stehen in `crates/dds4ccm/src/dlrl.rs`; die IDL von Annex B bindet `dds_dlrl.idl` über `crates/dds4ccm/idl/dds_rtf2_dlrl.idl` ein

**Crate-Mapping:**

| Spec-Bereich | Crate / Modul |
|---|---|
| §2 Conformance, Punkt 1 | `crates/dds4ccm/` (Test `tests/conformance.rs`) |
| §7.2.2 Normative DDS-DCPS Ports | `crates/dds4ccm/` |
| §7.3 DDS-DCPS Connectors | `crates/dds4ccm/src/connector.rs` |
| §7.4.1 DCPS Entities, §7.4.3 Use of QoS Profiles | `crates/dds4ccm/src/entities.rs` |
| §7.4.4 Threading Policy | `crates/dds4ccm/src/threading.rs` |
| §7 Datenmodell: Namen der DDS-DCPS-Ports und der Connectoren | `crates/ccm/src/dds4ccm.rs` |
| §7.4.2 DDS QoS Policies in XML | `crates/xml/src/qos_parser.rs`, `crates/xml/src/qos.rs` |
| §8 DDS-DLRL Extended Ports + Connectors | `crates/dds4ccm/src/dlrl.rs` |
| Annex A IDL3+ of DDS-DCPS Ports and Connectors | `crates/dds4ccm/idl/ccm_dds.idl` |
| Annex B IDL for DDS-DLRL Ports and Connectors | `crates/dds4ccm/idl/ccm_dds_dlrl.idl`, `crates/dds4ccm/idl/dds_rtf2_dlrl.idl` |
| Annex C XML Schema for QoS Profiles | `crates/xml/schemas/dds4ccm_qos_profile.xsd` |
| Annex D Default QoS Profile | `crates/xml/schemas/dds4ccm_default_qos_profile.xml` |
| Annex E QoS Policies for DDS Patterns | `crates/dds4ccm/src/profiles.rs`, `crates/dds4ccm/profiles/` |

Das Datenmodell der Ports und Connectoren (ihre Namen) liegt in
`crates/ccm/src/dds4ccm.rs`, die Connector-Fragmente von §7 mit ihrem
`Container` in `crates/dds4ccm/`; der CCM-Container-Stack `crates/corba-ccm/`
(siehe `omg-ccm-4.0.md`) bindet beide nicht ein. Der Leser der XML-QoS-Profile
liegt in `crates/xml/` (siehe `dds-xml-1.0.md` und `zerodds-xml-1.0.md`); die
Formen von DDS4CCM, die DDS-XML 1.0 nicht hat, liest er mit
`ReadOptions::accept_vendor_forms`. Die IDL von Annex A und Annex B liegt als
Datei in `crates/dds4ccm/idl/` und wird in den Tests durch `crates/idl-compose/`,
`crates/idl-parse/` und `crates/idl-lower/` gelesen; die von Annex B auch durch
omniidl.

---

## §1 Scope

### §1 Scope Statement

**Spec:** §1, S. 1 (PDF) — "This specification defines how CCM
components may interact using DDS and how related DDS entities may
be configured using CCM configuration mechanisms."

**Repo:** Interaktion über DDS: `crates/dds4ccm/` (Ports, Connectoren,
DCPS-Entities, Threading-Policy). Konfiguration über die Attribute der
Connectoren und QoS-Profile: `crates/dds4ccm/src/connector.rs`,
`crates/dds4ccm/src/entities.rs`, der Leser in `crates/xml/`. Die DLRL-Ports
und -Connectoren von §8: `crates/dds4ccm/src/dlrl.rs` über den DLRL-Entities
von `crates/dlrl/src/cache/`.

**Tests:** `crates/dds4ccm/tests/conformance.rs::dds4ccm_2_two_containers_exchange_state_and_events_through_the_normative_ports_and_connectors`, `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`; die Items von §7, §8 und den Anhängen.

**Status:** partial — der DCPS-Teil ist gebaut und über DDS getestet (§2, Punkt 1). Vom DLRL-Teil (§2, Punkt 2) sind gebaut und auf dem Linux-Testhost grün: `CacheOperation` (§8.2.1.1), die DLRL-Connectoren (§8.3), die DCPS-Entities eines DLRL-Ports (§8.4.1), die Zuteilung des QoS-Profils über `topic_filter` (§8.4.2) und die IDL von Annex B. Teilweise: die Home-Receptacles (§8.2.1.2) und damit die Kompositionsregel (§8.2.2), weil es in Rust keine erzeugten Home-Klassen gibt (`dlrl-1.2` §8.2.2, Punkt (3)). Für Punkt 1 ist die Beherbergung im CCM-Container von `crates/corba-ccm` nicht gebaut (siehe §2).

---

## §2 Conformance

### §2 Conformance Points

**Spec:** §2, S. 1 (PDF) — Zwei Conformance-Points:
1. "A CCM framework claiming conformance with this 'DDS for
   Lightweight CCM' specification shall support DDS-DCPS normative
   ports and connectors and their configuration."
2. "An optional compliance point [...] is the support for DLRL ports
   and connectors and their configuration."

**Repo:** Punkt 1: `crates/dds4ccm/` — die sechs Port-Typen von §7.2.2.2
(`src/ports.rs`) über den Basic Ports von §7.2.2.1, die Connectoren `DDS_Base`,
`DDS_TopicBase`, `DDS_State` und `DDS_Event` (`src/connector.rs`), ihre
DCPS-Entities und QoS-Profile (`src/entities.rs`), die Threading-Policy
(`src/threading.rs`), die IDL von Annex A (`idl/ccm_dds.idl`). Punkt 2:
`crates/dds4ccm/src/dlrl.rs` — `CacheOperation`, `DlrlPortType` mit den
Home-Receptacles, `DlrlPort`, `DlrlConnector` über `DDS_Base`, die
DCPS-Entities und das QoS-Profil eines DLRL-Ports, die IDL von Annex B
(`idl/ccm_dds_dlrl.idl`).

**Tests:** Punkt 1: `crates/dds4ccm/tests/conformance.rs::dds4ccm_2_two_containers_exchange_state_and_events_through_the_normative_ports_and_connectors`; jede Klausel einzeln in den Items von §7.2.2 bis §7.4.4 und Annex A, C, D. Punkt 2: `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`, je Klausel in den Items von §8.2.1.1 bis §8.4.2 und Annex B.

**Status:** partial — Punkt 1 ist erfüllt für den `Container` von `crates/dds4ccm`; die Beherbergung im CCM-Container von `crates/corba-ccm` ist nicht gebaut. Punkt 2 (optional) ist teilweise erfüllt. Auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131. Der Test von Punkt 1 ist ein Einsatz mit allem, was der Punkt nennt: zwei `Container`, die Connectoren `DDS_State` und `DDS_Event`, jeder der sechs Port-Typen, die Konfiguration über `domain_id`, `topic_name`, `key_fields` und ein `qos_profile` als `file:`-URL mit `topic_filter`, eine Threading-Policy des Containers. Eine Komponente des einen Containers schreibt einen Zustand über `DDS_Update` und Events über `DDS_Write`; Komponenten des anderen Containers beobachten über `DDS_Read`, `DDS_Get`, `DDS_Listen` und `DDS_StateListen` und empfangen über `DDS_Get` und `DDS_Listen`, über DDS. Die Komponenten sind Rust-Objekte im `Container` von `crates/dds4ccm`; der CCM-Container `crates/corba-ccm` beherbergt die Fragmente nicht (er hat keine Abhängigkeit auf DDS). Punkt 2: gebaut und grün sind `CacheOperation` (§8.2.1.1), die DLRL-Connectoren mit ihren Mirror-Ports und `error_listener` (§8.3), die DCPS-Entities eines DLRL-Ports (§8.4.1), das QoS-Profil mit `topic_filter` (§8.4.2) und die IDL von Annex B mit ihrem Abgleich gegen `DDS::Cache` und den Trait `CacheOperation`; zwei Container tauschen über je einen DLRL-Port Objekte über DDS aus (`dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`). Teilweise: ein Home-Receptacle ist die generische `ObjectHome` seiner Klasse, nicht eine vom DLRL-Werkzeug erzeugte Home-Klasse, die von `DDS::ObjectHome` erbt (§8.2.1.2, §8.2.2); für Rust gibt es keine erzeugten Klassen (`dlrl-1.2` §8.2.2, Punkt (3)). Die DLRL-Entities darunter sind in `crates/dlrl/src/cache/` gebaut (Dokument `dlrl-1.2`).

---

## §3 Normative References

**Spec:** §3, S. 1-2 (PDF) — CORBA 3.2 (Part 1/2/3, formal/2011-11-01..03),
CCM (= CORBA Part 3), D&C (formal/06-04-02), DDS 1.2 (formal/07-07-01),
XML Schema (W3C 2004).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

---

## §4 Terms and Definitions

**Spec:** §4, S. 2 (PDF) — Connector, Extended Port, Fragment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Begriffe.

---

## §5 Symbols

**Spec:** §5, S. 2 (PDF) — CCM/CIF/CORBA/DCPS/DDS/DLRL/IDL/UML/XML.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Abkürzungen.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 2 (PDF) — "None in this specification."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — "None in this specification."

### §6.2 Acknowledgements

**Spec:** §6.2, S. 3 (PDF) — Editorial.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Danksagung.

---

## §7 DDS-DCPS Extended Ports and Connectors

### §7.1 Introduction

#### §7.1.1 Rationale for DDS Extended Ports and Connectors Definition

**Spec:** §7.1.1, S. 5 (PDF) — Begründung warum DDS-spezifische
Connectors statt generic CCM-Receptacles.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Begründung.

#### §7.1.2 From Connector-Oriented Modeling to Connectionless Deployment

**Spec:** §7.1.2, S. 6 (PDF) — Pattern: Connector im Design,
DCPS-Topic im Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Erläuterung des Unterschieds zwischen Modell und Ausführung; keine Anforderung.

### §7.2 DDS-DCPS Extended Ports

#### §7.2.1 Design Rules

**Spec:** §7.2.1, S. 6-7 (PDF) — vier Design-Sub-Sections (§7.2.1.1
Parameterization, §7.2.1.2 Basic Ports Definition, §7.2.1.3 Interface
Design, §7.2.1.4 Simplicity vs Richness Trade-off).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Entwurfsregeln der IDL; die normativen Ports stehen in §7.2.2 und Annex A.

#### §7.2.2 Normative DDS-DCPS Ports

#### §7.2.2.1 DDS-DCPS Basic Port Interfaces

**Spec:** §7.2.2.1, S. 8-17 (PDF) — die Interfaces der Basic Ports:
`InstanceHandleManager`, `Writer`, `Updater` (§7.2.2.1.1); `ReadInfo`,
`QueryFilter`, `Reader`, `Getter`, `Listener`, `StateListener` und
Figure 7.2 (§7.2.2.1.2); `DataListenerControl`, `StateListenerControl`
(§7.2.2.1.3); `ContentFilterSetting` (§7.2.2.1.4); `PortStatusListener`,
`ConnectorStatusListener` (§7.2.2.1.5).

**Repo:** Krate `zerodds-dds4ccm`. Die IDL als Typen und Traits:
`crates/dds4ccm/src/ccm_dds.rs` (`ReadInfo`, `QueryFilter`, `AccessStatus`,
`InstanceStatus`, `ListenerMode`, `UNLIMITED`, die Exceptions
`AlreadyCreated`, `NonExistent`, `InternalError`, `NonChangeable`, die
Interfaces ohne Datentyp), `crates/dds4ccm/src/ccm_dds_typed.rs` (die Interfaces von
`CCM_DDS::Typed<T, TSeq>`). Die Connector-Fragmente über den DCPS-Entities:
`crates/dds4ccm/src/writer.rs::{WriterFragment, UpdaterFragment}`,
`crates/dds4ccm/src/reader.rs::{ReaderFragment, GetterFragment}`,
`crates/dds4ccm/src/reader_core.rs`, `crates/dds4ccm/src/instance_status.rs`
(Figure 7.2), `crates/dds4ccm/src/listener.rs`,
`crates/dds4ccm/src/status.rs`,
`crates/dcps/src/subscriber.rs::DataReader::read_instance_w_condition`. Die
Namen der Interfaces im Datenmodell: `crates/ccm/src/dds4ccm.rs::BasicPortKind`.

**Tests:** `crates/dds4ccm/tests/publishing_ports.rs::{dds4ccm_7_2_2_1_1_instance_handle_manager_registers_and_unregisters_through_dds, dds4ccm_7_2_2_1_1_write_one_with_a_handle_that_contradicts_the_key_is_internal_error, dds4ccm_7_2_2_1_1_write_many_stops_at_the_first_error_and_reports_its_index, dds4ccm_7_2_2_1_1_coherent_write_brackets_the_batch, dds4ccm_7_2_2_1_1_create_one_of_an_existing_instance_is_already_created, dds4ccm_7_2_2_1_1_delete_one_disposes_and_update_one_needs_an_existing_instance, dds4ccm_7_2_2_1_1_update_many_reports_every_missing_index, dds4ccm_7_2_2_1_1_is_global_scope_checks_the_lifecycle_in_the_data_space, dds4ccm_7_2_2_1_1_writer_and_updater_inherit_the_instance_handle_manager}`, `crates/dds4ccm/tests/subscribing_ports.rs::{dds4ccm_7_2_2_1_2_read_last_returns_one_sample_per_instance_and_read_all_every_sample, dds4ccm_7_2_2_1_2_a_read_returns_seen_samples_again_and_alive_instances_only, dds4ccm_7_2_2_1_2_read_one_last_of_an_unknown_instance_is_non_existent, dds4ccm_7_2_2_1_2_read_one_of_a_keyless_topic_reads_the_one_instance, dds4ccm_7_2_2_1_2_a_query_filters_every_read, dds4ccm_7_2_2_1_2_get_one_returns_false_after_time_out, dds4ccm_7_2_2_1_2_get_many_respects_max_delivered_data, dds4ccm_7_2_2_1_2_listener_modes, dds4ccm_7_2_2_1_2_state_listener_calls_on_creation_on_one_update_on_deletion, dds4ccm_7_2_2_1_2_instance_status_follows_figure_7_2, dds4ccm_7_2_2_1_2_an_instance_leaving_and_reentering_the_query_is_filtered_out_and_in, dds4ccm_7_2_2_1_2_a_fresh_sample_the_query_rejected_is_not_gotten_under_a_later_query, dds4ccm_7_2_2_1_2_a_listener_gets_the_exit_from_the_query_and_the_reentry, dds4ccm_7_2_2_1_3_is_filter_interpreted_false_makes_exit_and_reentry_updates, dds4ccm_7_2_2_1_3_is_filter_interpreted_true_makes_exit_a_deletion_and_reentry_a_creation, dds4ccm_7_2_2_1_3_listener_control_defaults, dds4ccm_7_2_2_1_4_set_filter_parameters, dds4ccm_7_2_2_1_5_status_listeners, dds4ccm_7_2_2_1_5_each_dds_status_goes_to_the_operation_of_its_listener}`, `crates/dds4ccm/src/instance_status.rs::{tests::dds4ccm_7_2_2_1_2_figure_7_2_created_updated_deleted, tests::dds4ccm_7_2_2_1_2_figure_7_2_a_changed_disposed_generation_count_creates_again, tests::dds4ccm_7_2_2_1_2_a_sample_without_data_is_passed_only_as_a_deletion, tests::dds4ccm_7_2_2_1_2_data_read_after_the_disposal_keeps_the_status_of_its_time, tests::dds4ccm_7_2_2_1_2_a_sample_read_again_keeps_its_status_and_does_not_move_the_state, tests::dds4ccm_7_2_2_1_2_an_existing_instance_leaves_and_reenters_the_query, tests::dds4ccm_7_2_2_1_2_an_instance_outside_the_query_is_deleted_or_created_again, tests::dds4ccm_7_2_2_1_2_deliver_asks_the_query_for_fresh_data_and_recalls_the_reentry, tests::dds4ccm_7_2_2_1_2_access_status_follows_the_sample_state, tests::dds4ccm_7_2_2_1_2_what_is_returned_is_ordered_by_instance, tests::dds4ccm_7_2_2_1_2_instances_the_reader_purged_are_forgotten}`, `crates/dds4ccm/src/listener.rs::{tests::dds4ccm_7_2_2_1_3_is_filter_interpreted_decides_what_a_filtered_instance_triggers, tests::dds4ccm_7_2_2_1_3_an_instance_told_deleted_at_its_exit_is_not_deleted_twice, tests::dds4ccm_7_2_2_1_2_state_listener_groups_the_updates_by_the_mode}`, `crates/dds4ccm/src/writer.rs::{tests::dds4ccm_7_2_2_1_1_the_index_of_an_exception_is_the_position_in_the_sequence, tests::dds4ccm_7_2_2_1_1_the_later_of_the_observer_s_changes_is_its_latest}`, `crates/dds4ccm/src/reader_core.rs::{tests::dds4ccm_7_2_2_1_2_read_last_keeps_the_last_sample_of_each_instance, tests::dds4ccm_7_2_2_1_2_no_data_is_an_empty_sequence_and_an_error_stays_one}`, `crates/dds4ccm/src/ccm_dds.rs::{tests::dds4ccm_annex_a_unlimited_is_zero_and_the_defaults_are_those_of_the_idl, tests::dds4ccm_annex_a_internal_error_carries_the_dds_return_code_and_the_index}`, `crates/dcps/tests/read_api_spec.rs::dcps_2_2_2_5_3_14_read_instance_w_condition_reads_one_instance_through_the_condition`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_2_2_1_basic_port_interfaces_have_the_names_of_the_idl`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48, `-p zerodds-dcps --test read_api_spec` 7 von 7. `INSTANCE_FILTERED_OUT` und `INSTANCE_FILTERED_IN` berechnet das Fragment für die Query des Readers des Ports, auf dem Pfad frischer Daten (`Getter`, `Listener`, `StateListener`): das erste frische Sample einer Instanz, die für die Komponente existiert, das die Query abweist, ist `INSTANCE_FILTERED_OUT` und wird mit seinen Daten geliefert; das nächste Sample der Instanz, das die Query passiert, ist `INSTANCE_FILTERED_IN`. `is_filter_interpreted` TRUE macht daraus `on_deletion` und `on_creation`, FALSE Updates; die Tests prüfen beide Werte über DDS. Für den Content-Filter des Ports (Attribut `filter`) gibt es die beiden Status nicht: ein Sample, das ein ContentFilteredTopic abweist, erreicht den DataReader nicht. Abweichung von einem DDS-Read über eine QueryCondition: das Fragment liest jedes `NOT_READ`-Sample und wertet die Query selbst aus. Ein frisches Sample, das die Query abweist, ist danach `READ`; `Getter` und Listener liefern es auch unter einer später gesetzten Query nicht mehr, `Reader` liefert es als `ALREADY_SEEN`. `INCONSISTENT_TOPIC`: `crates/dds4ccm/src/status.rs::TopicStatusListener` ruft `on_inconsistent_topic` auf. Kein Port setzt diesen TopicListener; das Topic gehört dem Connector, der ihn auf sein Topic setzt (§7.3.1, dort über DDS getestet). Die Tests lösen `REQUESTED_DEADLINE_MISSED`, `OFFERED_DEADLINE_MISSED`, die beiden `INCOMPATIBLE_QOS` und die beiden `MATCHED` über DDS aus; für `SAMPLE_LOST`, `SAMPLE_REJECTED`, `LIVELINESS_CHANGED` und `LIVELINESS_LOST` ruft der Test die DDS-Listener des Ports auf, wie DDS sie aufruft, für `INCONSISTENT_TOPIC` den `TopicStatusListener`.

#### §7.2.2.2 DDS-DCPS Extended Ports

**Spec:** §7.2.2.2, S. 18-19 (PDF) — sechs Port-Typen aus den Basic Ports:
`DDS_Write`, `DDS_Update`, `DDS_Read`, `DDS_Get`, `DDS_Listen`,
`DDS_StateListen`; die Ports der Subscriber-Seite mit dem Attribut `filter`
(`setraises(NonChangeable)`), `filter_config`, `dds_entity` und `status`.

**Repo:** `crates/dds4ccm/src/ports.rs::{DdsWrite, DdsUpdate, DdsRead, DdsGet,
DdsListen, DdsStateListen}` mit den Basic Ports unter den Namen der
Spezifikation (`data`, `fresh_data`, `data_control`, `data_listener`,
`filter_config`, `dds_entity`, `status`); `configuration_complete` legt den
DataReader des Ports an, bei einem Filter mit Ausdruck auf einem
ContentFilteredTopic, danach ist `filter` `NonChangeable`;
`crates/dds4ccm/src/receptacle.rs`. Die Port-Typen und ihre Basic Ports im
Datenmodell: `crates/ccm/src/dds4ccm.rs::ExtendedPortKind::basic_ports`.

**Tests:** `crates/dds4ccm/tests/extended_ports.rs::{dds4ccm_7_2_2_2_each_port_type_has_the_basic_ports_of_the_specification, dds4ccm_7_2_2_2_filter_is_non_changeable_after_configuration, dds4ccm_7_2_2_2_filter_selects_what_the_reader_receives, dds4ccm_7_2_2_2_dds_entity_is_the_underlying_reader, dds4ccm_7_2_2_2_a_removed_port_deletes_its_reader}`, `crates/dds4ccm/src/receptacle.rs::tests::dds4ccm_7_2_2_2_a_receptacle_holds_one_connection`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_2_2_2_port_types_are_made_of_the_basic_ports_of_the_specification`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --test extended_ports` 5 von 5, `-p zerodds-ccm` 48 von 48.

### §7.3 DDS-DCPS Connectors

#### §7.3.1 Base Connectors

**Spec:** §7.3.1, S. 20 (PDF) — `DDS_Base`: `uses ConnectorStatusListener
error_listener`, Attribute `domain_id` und `qos_profile` ("File URL or XML
string"). `DDS_TopicBase : DDS_Base`: Attribute `topic_name` und `key_fields`.
Alle vier Attribute `setraises(NonChangeable)`: "Any attempt to change those
attributes once the configuration is complete will raise a NonChangeable
exception."

**Repo:** `crates/dds4ccm/src/connector.rs::{DdsBase, DdsTopicBase,
ConnectorError}`; `crates/dds4ccm/src/entities.rs::{Profile, ProfileError}`
liest `qos_profile`; `crates/dds4ccm/src/status.rs::TopicStatusListener` ist
der TopicListener, den der Connector auf sein Topic setzt. Datenmodell:
`crates/ccm/src/dds4ccm.rs::{ConnectorKind, ConnectorAttribute}`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_1_the_attributes_raise_non_changeable_once_the_configuration_is_complete, dds4ccm_7_3_1_qos_profile_is_a_file_url_or_the_xml_string, dds4ccm_7_3_1_a_profile_that_cannot_be_read_leaves_the_connector_unconfigured, dds4ccm_7_3_1_topic_name_and_key_fields, dds4ccm_7_3_1_error_listener_gets_the_inconsistent_topic_and_the_incompatible_qos, dds4ccm_7_3_1_two_connectors_of_one_topic_both_get_the_inconsistent_topic, dds4ccm_7_3_1_a_topic_that_exists_must_have_the_topic_qos_of_the_profile, dds4ccm_7_3_1_key_fields_of_a_type_with_its_type_object_are_its_key_members, dds4ccm_7_3_1_error_listener_gets_the_statuses_of_the_observer_of_a_global_update_port}`, `crates/dds4ccm/tests/subscribing_ports.rs::dds4ccm_7_2_2_1_5_each_dds_status_goes_to_the_operation_of_its_listener`, `crates/dds4ccm/src/connector.rs::{tests::dds4ccm_7_3_1_key_fields_are_compared_with_the_key_members_the_type_names, tests::dds4ccm_7_3_1_key_fields_of_a_type_that_does_not_name_its_key_are_checked_for_presence, tests::dds4ccm_7_3_1_a_topic_connector_is_completed_as_one, tests::dds4ccm_7_3_1_a_connector_error_says_what_is_wrong}`, `crates/dds4ccm/src/entities.rs::{tests::dds4ccm_7_3_1_an_xml_string_is_read_and_its_last_profile_is_the_profile, tests::dds4ccm_7_3_1_the_empty_value_is_no_profile_and_every_qos_is_the_default, tests::dds4ccm_7_3_1_a_container_without_policies_is_the_default_qos, tests::dds4ccm_7_3_1_a_file_url_is_parsed_into_path_and_profile, tests::dds4ccm_7_3_1_what_is_no_profile_document_is_an_error, tests::dds4ccm_7_3_1_the_profile_of_a_document_with_libraries, tests::dds4ccm_7_3_1_a_topic_differs_in_the_policies_the_profile_sets_to_another_value}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_1_base_connectors_have_the_attributes_of_the_idl`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48. Ein Setter nach `configuration_complete` gibt `NonChangeable` zurück; ein fehlgeschlagenes `configuration_complete` lässt die Attribute änderbar. `INCONSISTENT_TOPIC` und die beiden `INCOMPATIBLE_QOS` erreichen `error_listener` über DDS. Dorthin meldet auch der DataReader, auf dem ein `DDS_Update`-Port mit `is_global_scope` die Instanzen nachschlägt (`REQUESTED_INCOMPATIBLE_QOS` über DDS getestet). Dieser Port hat keinen `PortStatusListener`: `REQUESTED_DEADLINE_MISSED` und `SAMPLE_LOST` dieses DataReaders gehen an `on_unexpected_status`; dafür ruft der Test den DDS-Listener auf, wie DDS ihn aufruft. Festlegungen, wo die Spezifikation nichts sagt: (1) `qos_profile`: ein Wert, der mit `<` beginnt, ist der XML-Text; eine `file:`-URL (`file:///pfad`, `file:/pfad`, `file://localhost/pfad`, mit `%xx`) nennt die Datei; unter Windows nennt `file:///C:/pfad` den Pfad `C:/pfad` (RFC 8089 Appendix E.2); der leere Wert ist kein Profil, die Entities haben die Default-QoS von DDS; jeder andere Wert ist ein Fehler. Enthält das Dokument mehrere Profile, gilt das letzte direkt unter `<dds>` (§7.4.2.3.1: ein Profil erbt nur von vorher gelesenen). Hat das Dokument Profile nur in `<qos_library>`-Elementen (die Form von DDS-XML 1.0), gilt das letzte Profil der letzten Bibliothek, die eines hat. Hat es beides, gehen die Profile direkt unter `<dds>` vor, gleich wo die Bibliotheken stehen. Das Fragment der URL nennt ein anderes Profil (`#Profil`, `#Bibliothek::Profil`). Gelesen wird mit den Formen von DDS4CCM, die DDS-XML 1.0 nicht hat (Profil direkt unter `<dds>`, `domainparticipant_qos`). (2) `key_fields`: der Schlüssel eines Topics ist der seines Datentyps. Das Attribut wird gegen den Typ geprüft: mit den Namen der Key-Member, wo der Typ sein vollständiges `TypeObject` trägt und keinen Basistyp hat, sonst darauf, ob der Typ einen Schlüssel hat. Ein Widerspruch ist `ConnectorError::KeyFields`. Beide Wege sind über DDS getestet: mit einem Typ ohne `TypeObject` und mit einem Typ, der sein `TypeObject` trägt. (3) Nutzen zwei Connectoren in einem DomainParticipant ein Topic gleichen Namens und Datentyps, teilen sie das Topic und seinen TopicListener; der zuletzt entfernte löscht das Topic. Ein Topic, das im DomainParticipant schon existiert, behält seine QoS: das Topic eines anderen Connectors ebenso wie ein Topic, das die Anwendung erzeugt hat. Es dient dem Connector, wenn jede Policy, die das `topic_qos` seines Profils setzt, dort denselben Wert hat; eine nicht gesetzte Policy ist nicht angefordert. Sonst weist der Connector den Port mit `ConnectorError::TopicQos` ab, das die abweichenden Policies nennt, und erzeugt nichts für ihn. Nicht gelaufen: eine `file:`-URL mit Laufwerksbuchstabe unter Windows. Die Abbildung des Pfads ist auf dem Linux-Testhost getestet, das Lesen der Datei unter Windows nicht; die Windows-Jobs von `ci/platform_jobs.py` bauen `zerodds-dds4ccm` nicht.

#### §7.3.2 Pattern State Transfer

**Spec:** §7.3.2, S. 20-21 (PDF) — `connector DDS_State : DDS_TopicBase` mit
den Mirror-Ports `observable` (`DDS_Update`), `passive_observer` (`DDS_Read`),
`pull_observer` (`DDS_Get`), `push_observer` (`DDS_Listen`) und
`push_state_observer` (`DDS_StateListen`). "Typically, with this pattern,
HISTORY QoS should be set to KEEP_LAST."

**Repo:** `crates/dds4ccm/src/connector.rs::DdsState`; Datenmodell:
`crates/ccm/src/dds4ccm.rs::ConnectorKind::State`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_2_state_connector_the_observable_reaches_the_four_observers, dds4ccm_7_3_2_and_7_3_3_history_is_that_of_the_pattern_unless_the_profile_sets_it}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_2_and_7_3_3_pattern_connectors_have_the_mirror_ports_of_the_idl`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48. Ein Mirror-Port ist eine Methode des Connectors unter dem Namen der Spezifikation. `observable(component, is_global_scope)` gibt den Port `DDS_Update` zurück; die vier Observer-Methoden bekommen den Port der Komponente, dessen Attribut `filter` und Threading-Policy gesetzt sind, und schließen seine Konfiguration ab. Der Test schreibt über `observable` und liest auf allen vier Observer-Ports über DDS. HISTORY ist KEEP_LAST, wo das QoS-Profil HISTORY nicht setzt. Die QoS-Tabellen von Annex E wendet der Connector nicht von sich aus an; sie liegen als Profil `profiles::STATE_TRANSFER` für das Attribut `qos_profile` vor (Annex E).

#### §7.3.3 Pattern Event Transfer

**Spec:** §7.3.3, S. 21 (PDF) — `connector DDS_Event : DDS_TopicBase` mit den
Mirror-Ports `supplier` (`DDS_Write`), `pull_consumer` (`DDS_Get`) und
`push_consumer` (`DDS_Listen`). "Typically, with this pattern, HISTORY QoS
should be set to KEEP_ALL."

**Repo:** `crates/dds4ccm/src/connector.rs::DdsEvent`; Datenmodell:
`crates/ccm/src/dds4ccm.rs::ConnectorKind::Event`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_3_event_connector_the_supplier_reaches_both_consumers, dds4ccm_7_3_2_and_7_3_3_history_is_that_of_the_pattern_unless_the_profile_sets_it}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_2_and_7_3_3_pattern_connectors_have_the_mirror_ports_of_the_idl`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48. `supplier(component)` gibt den Port `DDS_Write` zurück; `pull_consumer` und `push_consumer` bekommen den Port der Komponente. Der Test sendet über `supplier`; beide Consumer erhalten jedes Event über DDS. HISTORY ist KEEP_ALL, wo das QoS-Profil HISTORY nicht setzt. Die QoS-Tabellen von Annex E wendet der Connector nicht von sich aus an; sie liegen als Profil `profiles::EVENT_TRANSFER` für das Attribut `qos_profile` vor (Annex E).

### §7.4 Configuration and QoS Support

#### §7.4.1 DCPS Entities

**Spec:** §7.4.1, S. 21 (PDF) — die Connector-Fragmente erzeugen die
DDS-Entities. Ein Port hat einen DataReader und/oder DataWriter, "entirely
dedicated to their port". Subscriber, Publisher und DomainParticipant "may be
allocated to the port or to the component [...] or to the container";
"components and containers can be given DDS entities that can be used by the
infrastructure for servicing embedded ports if they meet the port
requirements".

**Repo:** `crates/dds4ccm/src/entities.rs::{Container, Component}` (die
Default-Entities je Domain, gegeben oder aus dem Profil erzeugt, und die
Zuteilung `Component::place`); `crates/dds4ccm/src/connector.rs` (Topic,
DataWriter und DataReader je Port, `remove`).

**Tests:** `crates/dds4ccm/tests/entities.rs::{dds4ccm_7_4_1_each_port_has_its_reader_or_writer_and_shares_the_entities_of_the_container, dds4ccm_7_4_1_a_container_and_a_component_can_be_given_their_entities, dds4ccm_7_4_1_a_given_participant_serves_the_ports_it_suits, dds4ccm_7_4_1_a_container_whose_defaults_serve_a_port_is_not_removed, dds4ccm_7_4_1_a_port_removed_before_its_connector_leaves_the_connector_removable, dds4ccm_7_4_1_a_removal_that_failed_is_tried_again, dds4ccm_7_4_1_a_container_creates_its_participants_with_the_standard_runtime_configuration, dds4ccm_7_4_3_a_port_the_defaults_do_not_suit_gets_dedicated_entities}`, `crates/dds4ccm/tests/connectors.rs::dds4ccm_7_4_1_a_listener_called_at_the_connection_may_use_the_connector`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 117 von 117, `-p zerodds-ccm` 48 von 48. Jeder Port hat seinen DataReader oder DataWriter. Ein Port bekommt, in dieser Reihenfolge: Publisher oder Subscriber des Containers mit dem DomainParticipant des Containers; die der Komponente mit dem DomainParticipant, in dem sie liegen; einen eigenen Publisher oder Subscriber im ersten Default-DomainParticipant, der passt; einen eigenen DomainParticipant mit eigenem Publisher oder Subscriber. Ein Container hat immer Defaults (ohne Profil mit der Default-QoS von DDS); ohne Profile teilen alle Ports eines Containers dessen drei Entities. `set_default_entities` gibt einem Container oder einer Komponente DomainParticipant, Publisher und Subscriber; sie werden nicht gelöscht. `remove` des Connectors löscht, was seine Fragmente erzeugt haben, `Container::remove` die erzeugten Defaults. Ein `remove` des Ports oder des Connectors, das scheitert (etwa an einer ReadCondition der Anwendung auf dem DataReader), behält, was es nicht löschen konnte; der nächste Aufruf versucht es erneut, ein gleichzeitiger Aufruf wartet auf den ersten. Die Daten, die ein DataReader beim Verbinden seines Push-Ports schon hält, gehen an den Listener der Komponente, nachdem der Connector das Fragment abgeschlossen und seine Sperren freigegeben hat: ein Listener kann dabei Ports mit demselben Connector verbinden oder ihn entfernen (Test über DDS). `Container::remove` prüft zuerst: solange ein Port seinen DataReader oder DataWriter in einem erzeugten Default-Publisher oder Default-Subscriber hat, oder in einem eigenen Publisher oder Subscriber in einem erzeugten Default-DomainParticipant, ist das Ergebnis `PRECONDITION_NOT_MET`; nichts wird gelöscht, die Defaults bleiben, der Port arbeitet weiter (Test über DDS). Ein erzeugter Default-DomainParticipant wird ohne seinen Inhalt gelöscht: liegt in ihm noch ein Topic, bleibt er Default seiner Domain und der nächste Aufruf löscht ihn. `Container::new` erzeugt die DomainParticipants mit der Standard-Laufzeitkonfiguration, `Container::with_runtime_config` mit der gegebenen; beide sind über DDS getestet. Die Tests lesen die Zuteilung an den Entities ab (Subscriber und Publisher des Readers und Writers, deren DomainParticipant). Was "passt", steht bei §7.4.3.

#### §7.4.2 DDS QoS Policies in XML

#### §7.4.2.1 XML File Syntax

**Spec:** §7.4.2.1, S. 22 (PDF) — UTF-8; Werte ohne führende und folgende Leerzeichen; Werte unterscheiden Groß- und Kleinschreibung; Kommentare `<!-- -->`; Wurzel `<dds>`. Table 7.1: Boolean "yes, 1, true or BOOLEAN_TRUE / no, 0, false or BOOLEAN_FALSE", "Not case-sensitive"; Enum als Zeichenkette; Long und UnsignedLong dezimal oder hexadezimal, `LENGTH_UNLIMITED`.

**Repo:** `crates/xml/src/parser.rs` (UTF-8, Kommentare), `crates/xml/src/zerodds_xml.rs::parse_dds_xml_with` (Wurzel `<dds>`), `crates/xml/src/qos_parser.rs` (`bool_value`, `table_7_1_hex`, `limit_value`, `positive_long`, `idl_long_value`, `partition_name`, die Enum-Literale je Policy), `crates/xml/src/types.rs` (`parse_bool_dds4ccm`, `parse_long`, `parse_ulong`), `crates/xml/src/options.rs` (`VendorForms`, `Ctx::table_7_1_form_outside_schema`)

**Tests:** `crates/xml/src/parser.rs::tests::parse_minimal_document`, `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`, `crates/xml/src/parser.rs::tests::non_utf8_declared_encoding_rejected`, `crates/xml/src/parser.rs::tests::comments_stripped`, `crates/xml/src/parser.rs::tests::attributes_preserved`, `crates/xml/src/parser.rs::tests::invalid_xml_rejected`, `crates/xml/src/qos_parser.rs::tests::rejects_non_dds_root`, `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_1_leading_and_trailing_spaces_are_removed, dds4ccm_7_4_2_1_enums_are_case_sensitive_strings, dds4ccm_7_4_2_1_table_7_1_long_is_decimal_hexadecimal_or_length_unlimited, dds4ccm_7_4_2_1_the_root_is_dds, dds4ccm_7_4_2_1_table_7_1_the_default_reading_names_the_form}`, `crates/xml/tests/spec_items.rs::numbers_outside_the_schemas_types_are_refused`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548. UTF-8 (eine andere deklarierte Kodierung wird abgewiesen), Kommentare an jeder Stelle, die Wurzel `<dds>` (ein Profil oder ein einzelnes QoS als Wurzel wird abgewiesen; die Wurzel von Annex D mit Namensraum und `xsi:schemaLocation` wird gelesen). Leerzeichen am Anfang und Ende werden vor dem Lesen jedes Wertes entfernt; der Test liest ein Profil mit Leerzeichen, Tabulator und Zeilenumbruch um jeden Wert und vergleicht mit dem Profil ohne sie. Für Partitionsnamen behält DDS-XML 1.0 §7.1.4 die Leerzeichen als Teil der Zeichenkette; die Lesart von DDS4CCM (`ReadOptions::accept_vendor_forms`, so lesen die Connectoren von `crates/dds4ccm`) entfernt sie und meldet es. Enum-Literale unterscheiden Groß- und Kleinschreibung, eine Zahl ist kein Enum (`XmlError::BadEnum`, in beiden Lesarten). Long und UnsignedLong von Table 7.1: dezimal, hexadezimal, `LENGTH_UNLIMITED` in jedem Feld dieses Typs. Die hexadezimale Zahl und der negative Long stehen ebenso in Table 7.1 von DDS-XML 1.0 §7.1.4. Das Schema von DDS-XML 1.0 lässt sie in jedem Zahlenfeld einer QoS-Policy aus: `positiveInteger_UNLIMITED`, `nonNegativeInteger_Duration_*`, `xs:positiveInteger` und `xs:nonNegativeInteger` haben nur Dezimalziffern, `xs:nonNegativeInteger` keinen negativen Wert. Text und Schema von DDS-XML 1.0 widersprechen sich hier; die Vorgabe-Lesart folgt dem Schema. Eine Regel gilt für alle Zahlenfelder: die hexadezimale Zahl in `history/depth`, `durability_service/history_depth`, den Grenzen von `resource_limits` und `durability_service`, `resource_limits/initial_samples` und `initial_instances`, `sec`/`nanosec` einer Dauer, `ownership_strength/value` und `transport_priority/value`, dazu ein negativer Wert und `LENGTH_UNLIMITED` in den beiden letzten (in der IDL `long`), ist eine Herstellerform. Die Lesart von DDS4CCM liest und meldet sie (eine Meldung je Element), die Vorgabe-Lesart weist sie mit `XmlError::VendorForm` ab; die Meldung nennt Table 7.1 beider Spezifikationen und den Typ des Schemas. Die Booleans von DDS4CCM sind dagegen keine Form von DDS-XML 1.0, ihre Meldung sagt das. Werte außerhalb des Bereichs, den die IDL dem Feld gibt, bleiben ein Fehler (eine Ressourcengrenze oder `history/depth` 0 oder `0x80000000`, eine Dauer mit `0x80000000` Sekunden, mehr als 32 Bit). Booleans: Item §7.4.2.1 Table 7.1. "Legal values are alphanumeric characters" prüft der Leser nicht eigens: die Beispiele der Spezifikation selbst schreiben `_`, `*` und `-` in Werten; jeder Wert wird nach seinem Typ gelesen.

#### §7.4.2.1 Table 7.1 — Boolean `yes`/`no`/`BOOLEAN_TRUE`/`BOOLEAN_FALSE` ohne Beachtung der Schreibweise

**Spec:** §7.4.2.1, Table 7.1, S. 22 (PDF) — "Boolean | yes, 1, true or BOOLEAN_TRUE: / no, 0, false or BOOLEAN_FALSE: | Not case-sensitive".

**Repo:** `crates/xml/src/types.rs::parse_bool_dds4ccm`, `crates/xml/src/qos_parser.rs::bool_value` (aufgerufen für `coherent_access`, `ordered_access`, `autoenable_created_entities`, `autodispose_unregistered_instances`)

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_1_table_7_1_boolean_forms_are_read_without_regard_to_case, dds4ccm_7_4_2_1_table_7_1_other_boolean_text_is_refused, dds4ccm_7_4_2_1_table_7_1_the_default_reading_names_the_form}`, `crates/xml/src/types.rs::tests::dds4ccm_7_4_2_1_table_7_1_bool_forms_in_any_case`, `crates/xml/src/qos_parser.rs::tests::boolean_case_sensitive_rejected`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548. Die vier Boolean-Felder eines QoS-Profils lesen `yes`, `1`, `true`, `BOOLEAN_TRUE` und `no`, `0`, `false`, `BOOLEAN_FALSE` in jeder Schreibweise; der Test prüft jede Form in allen vier Feldern mit dem Wert in `EntityQos`. `true`, `1`, `false`, `0` sind auch Formen von DDS-XML 1.0 §7.1.4 und werden ohne Meldung gelesen; die übrigen sind dort keine Form: die Lesart von DDS4CCM (`ReadOptions::accept_vendor_forms`) liest sie und meldet jede mit dem Pfad ihres Elements, die Vorgabe-Lesart weist sie mit `XmlError::VendorForm` ab und nennt Element und Text. Ein anderer Text (`on`, `y`, `2`, leer) endet in beiden Lesarten mit `XmlError::ValueOutOfRange`. Annex C typisiert die Felder `xs:boolean`; `xmllint` weist `yes` und `TRUE` ab, der Leser folgt dem Text von Table 7.1 (Zeilen `boolean_yes` und `boolean_upper_case` in `dds4ccm_annex_c_what_the_schema_rejects_we_reject`).

#### §7.4.2.2 Entity QoS

**Spec:** §7.4.2.2, S. 22-25 (PDF) — die sechs QoS-Container
(`participant_qos`, `topic_qos`, `publisher_qos`, `subscriber_qos`,
`datawriter_qos`, `datareader_qos`), ihre Policies "1-to-1" aus der IDL von DDS
(§7.4.2.2.1), Sequenzen von Oktetten als Liste dezimaler oder hexadezimaler
Werte (§7.4.2.2.2), Sequenzen von Strings als `<element>`-Liste (§7.4.2.2.3),
Enums als Zeichenkette (§7.4.2.2.4), Dauern mit `sec` und `nanosec` und ihren
Konstanten (§7.4.2.2.5); ein QoS mit `name` erbt über `base_name`.

**Repo:** `crates/xml/src/qos_parser.rs` (die sechs Container, die Policies,
`vendor_container_name` für `<domainparticipant_qos>`),
`crates/xml/src/qos.rs::{QosProfile, EntityQos}`, `crates/xml/src/types.rs`
(Dauern, Zahlen, Oktette).

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_c_what_the_schema_accepts_we_read, dds4ccm_annex_c_what_the_schema_rejects_we_reject, dds4ccm_7_4_2_the_forms_of_the_text_the_schema_rejects_have_their_values, dds4ccm_7_4_2_2_2_octets_are_the_list_of_the_text_or_the_base64_of_the_schema, dds4ccm_7_4_2_3_profiles_inherit_select_by_topic_filter_and_stand_for_a_single_qos}`, `crates/xml/src/qos.rs::tests::{entity_qos_default_is_all_none, entity_qos_into_writer_uses_defaults_for_unset, entity_qos_into_reader_uses_defaults_for_unset, merge_override_replaces_field, merge_none_does_not_clobber}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548. Der Test liest jede Policy, die das Schema von Annex C deklariert, in jedem der sechs Container und vergleicht jedes Feld mit dem Wert des Dokuments; `xmllint` bestätigt, dass das Schema das Dokument annimmt. Widersprüche in der Spezifikation und wie der Leser sie auflöst: (1) §7.4.2.2 nennt `<participant_qos>`, Annex C und Annex D `<domainparticipant_qos>`: beide werden in denselben Container gelesen. (2) §7.4.2.2.2 schreibt Oktette als Liste, Annex C typisiert `xs:base64Binary`: ein Inhalt in der Form der Liste ist die Liste, jeder andere Base64; ein Inhalt, der beides sein kann, ist die Liste des Textes. (3) §7.4.2.2.5 nennt `DURATION_ZERO_SEC` und `DURATION_ZERO_NSEC`, die Muster von Annex C nicht: gelesen. (4) §7.4.2.2 nennt Namen "by its fully qualified name in C++ style", das Muster `elementName` von Annex C hat weder `_` noch `:`: Namen werden gelesen, wie sie stehen. (5) `transport_priority/value` ist in der IDL ein `long` und Table 7.1 gibt einem Long die negative Hälfte, Annex C typisiert `xs:nonNegativeInteger`: ein negativer Wert wird gelesen (als Herstellerform gemeldet; die Vorgabe-Lesart weist ihn ab wie das Schema von DDS-XML 1.0), ebenso für `ownership_strength/value`. `<domainparticipant_qos>` ist keine Form von DDS-XML 1.0; der Leser nimmt sie mit `ReadOptions::accept_vendor_forms` an, die Connectoren von `crates/dds4ccm` lesen so. Die Schema-Vergleiche brauchen `xmllint` (libxml2-utils); ohne ihn kehren diese Tests ohne Prüfung zurück (`[skip]`). `ci/Dockerfile.rust` installiert es im Image `ci-rust` der Merge-Lane; bis dieses Image neu gebaut ist, kehren die Vergleiche in CI ungeprüft zurück.

#### §7.4.2.3 QoS Profiles

**Spec:** §7.4.2.3, S. 25-27 (PDF) — `<qos_profile name="...">` mit den sechs
Containern; `base_name`: ein Profil erbt von einem vorher gelesenen Profil
(§7.4.2.3.1); `topic_filter` wählt unter mehreren QoS einer Art, ein QoS ohne
das Attribut hat den Filter `*` (§7.4.2.3.2); ein einzelnes QoS mit `name`
steht für ein Profil dieses Namens (§7.4.2.3.3).

**Repo:** `crates/xml/src/qos_parser.rs::parse_ccm_top_level_profile` (Profile
und einzelne QoS direkt unter `<dds>`), `crates/xml/src/qos.rs` (`base_name`,
`topic_filter`, Auswahl je Topic).

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_3_profiles_inherit_select_by_topic_filter_and_stand_for_a_single_qos, dds4ccm_7_4_2_3_the_forms_of_dds4ccm_are_read_on_request_and_reported}`, `crates/xml/src/zerodds_xml.rs::tests::ccm_profiles_directly_under_dds_are_a_vendor_form`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`, `crates/xml/src/qos.rs::tests::{library_profile_lookup, glob_star_matches_all, glob_prefix, glob_question_mark, glob_exact}`, `crates/dds4ccm/src/entities.rs::tests::dds4ccm_7_4_3_the_topic_filter_allocates_the_values_to_the_entities_of_a_topic`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548, `-p zerodds-dds4ccm --all-targets` 113 von 113. Die Tests lesen die Beispiele des Textes in der Form von DDS4CCM: ein Profil erbt nur von einem vorher gelesenen; `topic_filter` wählt, ein QoS ohne das Attribut kommt nach den anderen; ein einzelnes QoS steht für ein Profil seines Namens. Profile direkt unter `<dds>` sind keine Form von DDS-XML 1.0: der Leser weist sie ohne `ReadOptions::accept_vendor_forms` ab und nennt die Form, mit der Option meldet er jede. Widersprüche in der Spezifikation: (1) §7.4.2.3.2 schreibt `topic_filter="A*"`, das Muster `topicNameFilter` von Annex C hat kein `*`: gelesen. (2) Die Beispiele des Textes haben keinen Namensraum, Annex C verlangt `http://www.omg.org/dds/`: die Elemente werden nach ihrem lokalen Namen gelesen. (3) Das Beispiel von §7.4.2.3.3 schreibt `<writer_qos>` und nennt es gleichwertig zu `<datawriter_qos>`; kein anderer Ort der Spezifikation und nicht Annex C kennt den Tag: abgewiesen.

#### §7.4.3 Use of QoS Profiles

**Spec:** §7.4.3, S. 27 (PDF) — das Profil des Connectors "should contain all
values for initializing DDS Entities that are required by the connector";
`topic_filter` teilt bei mehreren Topics zu. Ein Profil einer Komponente
definiert "component's default DomainParticipant, Subscriber, and/or
Publisher", die bevorzugt werden, "if their setting is compatible with the
QoS requested in the connector's profile"; sonst "specific entities dedicated
to the 'non-compatible' port will be created"; `topic_qos`, `datareader_qos`
und `datawriter_qos` dieses Profils werden ignoriert. Ein Profil eines
Containers definiert "container's defaults that should be used in priority,
if suitable".

**Repo:** `crates/dds4ccm/src/entities.rs::{Profile, Container::set_qos_profile,
Component::set_qos_profile, Component::place}`;
`crates/dds4ccm/src/connector.rs` (QoS von Topic, DataWriter und DataReader
aus dem Profil des Connectors).

**Tests:** `crates/dds4ccm/tests/entities.rs::{dds4ccm_7_4_3_the_connector_profile_initialises_every_entity, dds4ccm_7_4_3_a_port_the_defaults_do_not_suit_gets_dedicated_entities, dds4ccm_7_4_3_the_defaults_of_the_container_come_first_then_those_of_the_component, dds4ccm_7_4_3_a_component_profile_defines_the_default_participant_of_the_component, dds4ccm_7_4_3_what_cannot_be_configured_is_refused, dds4ccm_7_4_3_a_refused_port_keeps_no_entity_of_the_connector, dds4ccm_7_4_3_a_global_update_port_has_both_entities_in_its_dedicated_participant}`, `crates/dds4ccm/tests/connectors.rs::dds4ccm_7_3_1_a_topic_that_exists_must_have_the_topic_qos_of_the_profile`, `crates/dds4ccm/src/entities.rs::{tests::dds4ccm_7_4_3_the_topic_filter_allocates_the_values_to_the_entities_of_a_topic, tests::dds4ccm_7_4_3_a_default_entity_is_compatible_when_it_has_what_the_profile_sets}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48. Topic, DataWriter, DataReader, Publisher, Subscriber und DomainParticipant eines Ports haben jede Policy, die das Connector-Profil setzt, mit dessen Wert (Test liest sie an den Entities ab). DataWriter und DataReader erzeugt der Connector mit dem Profil. Publisher, Subscriber, DomainParticipant und Topic erzeugt er mit dem Profil, oder er nimmt ein vorhandenes Entity, das passt. Ein vorhandenes Topic, das nicht passt, weist den Port ab (§7.3.1, Festlegung 3): ein Topic gibt es je Name und Datentyp nur einmal im DomainParticipant. `topic_filter` wählt die Container nach dem Topic-Namen des Connectors. Festlegungen, wo die Spezifikation nichts sagt: (1) "compatible": ein Default-Entity passt, wenn jede Policy, die das Connector-Profil für diese Art Entity setzt, in der QoS des Entity denselben Wert hat; eine nicht gesetzte Policy ist nicht angefordert; ein DomainParticipant muss in der Domain des Connectors sein. (2) Reihenfolge: zuerst die Defaults des Containers ("in priority"), dann die der Komponente, dann ein eigenes Entity für den Port. (3) Eine Komponente hat einen Default-DomainParticipant, wenn ihr Profil ein `domainparticipant_qos` hat, und darin Publisher und Subscriber; hat ihr Profil nur `publisher_qos` oder `subscriber_qos`, liegt dieses Default-Entity im DomainParticipant des Containers. (4) Das Profil eines Containers oder einer Komponente ist fest, sobald ein Port deren Defaults angefragt hat (`LevelError::NonChangeable`). (5) Ein `DDS_Update`-Port mit `is_global_scope` hat zwei Entities: seinen DataWriter und den DataReader, auf dem er die Instanzen nachschlägt. Jedes wird wie oben zugeteilt; ein eigener DomainParticipant des Ports enthält beide, den DataReader in einem eigenen Subscriber.

#### §7.4.4 Other Configuration — Threading Policy

**Spec:** §7.4.4, S. 27 (PDF) — die Threading-Policy ist lokal zur Komponente
und wird am Port ("for all its facets"), an der Komponente oder am Container
gesetzt. Für ein Facet gilt der Threadpool des Ports; "if there is no port's
policy, the component's threadpool is used"; "if there is no component's one,
the container's threadpool is used"; "if there is no container's policy, then
the default is applied".

**Repo:** `crates/dds4ccm/src/threading.rs::{ThreadingPolicy, ThreadPool}`;
`set_threading_policy` an den Ports (`crates/dds4ccm/src/ports.rs`), an
`Component` und `Container` (`crates/dds4ccm/src/entities.rs`); die Facets
laufen über den Pool in `crates/dds4ccm/src/listener.rs` (`data_listener`) und
`crates/dds4ccm/src/status.rs` (`status`). Datenmodell:
`crates/ccm/src/dds4ccm.rs::ThreadingLevel`.

**Tests:** `crates/dds4ccm/tests/threading.rs::{dds4ccm_7_4_4_a_port_policy_wins, dds4ccm_7_4_4_without_a_port_policy_the_pool_of_the_component_is_used, dds4ccm_7_4_4_without_a_component_policy_the_pool_of_the_container_is_used, dds4ccm_7_4_4_without_any_policy_the_default_applies, dds4ccm_7_4_4_the_status_facet_is_called_on_the_pool_of_the_port, dds4ccm_7_4_4_a_policy_holds_for_the_facets_activated_after_it}`, `crates/dds4ccm/src/threading.rs::{tests::dds4ccm_7_4_4_a_pool_has_the_threads_of_its_policy_under_its_name, tests::dds4ccm_7_4_4_without_a_pool_a_facet_is_called_on_the_thread_of_the_event, tests::dds4ccm_7_4_4_the_facets_of_one_port_are_called_in_order_and_one_at_a_time, tests::dds4ccm_7_4_4_two_ports_of_one_pool_are_served_at_once, tests::dds4ccm_7_4_4_a_panicking_facet_does_not_cost_the_pool_its_thread, tests::dds4ccm_7_4_4_a_pool_let_go_of_runs_what_is_queued_and_ends_its_threads}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_4_4_threading_levels_fall_back_from_port_to_component_to_container`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-ccm` 48 von 48. Die Tests zeichnen den Thread auf, auf dem `on_one_data` und `on_requested_deadline_missed` aufgerufen werden. Festlegungen, wo die Spezifikation nichts sagt: (1) Eine Policy ist die Zahl der Threads eines Pools. (2) Facets eines Ports sind `data_listener` und `status`. `error_listener` ist ein Receptacle des Connectors und wird auf dem Thread aufgerufen, auf dem DDS den Status meldet. (3) Der Default ohne jede Policy: kein Pool; das Facet wird auf dem Thread aufgerufen, der das Ereignis bringt (der Thread, auf dem DDS meldet, oder der Thread des Aufrufers für Daten, die der Port beim Einschalten des Listeners schon hält). (4) Die Aufrufe eines Ports laufen nacheinander und in ihrer Reihenfolge, auch auf einem Pool mit mehreren Threads. (5) Die Policy eines Ports ist nach `configuration_complete` `NonChangeable`; die einer Komponente oder eines Containers gilt für die danach aktivierten Ports.

---

## §8 DDS-DLRL Extended Ports and Connectors

### §8.1 Design Principles

#### §8.1.1 Scope of DLRL Extended Ports

**Spec:** §8.1.1, S. 29 (PDF) — DLRL-Port-Scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Entwurfsprinzip; der normative Port steht in §8.2 und Annex B.

#### §8.1.2 Scope of DLRL Connectors

**Spec:** §8.1.2, S. 29 (PDF) — DLRL-Connector-Scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Entwurfsprinzip; die Connectoren stehen in §8.3 und Annex B.

### §8.2 DDS-DLRL Extended Ports

#### §8.2.1 DLRL Basic Ports

#### §8.2.1.1 Cache Operation

**Spec:** §8.2.1.1, S. 30 (PDF) — `local interface CacheOperation`: der operative Teil von `DDS::Cache` (Attribute `cache_usage`, `objects`, `updates_enabled`, `homes`, `sub_accesses`, `listeners`; Operationen `refresh`, `attach_listener`, `detach_listener`, `enable_updates`, `disable_updates`, `create_access`, `delete_access`), "mimic exactly the DDS::Cache ones".

**Repo:** `crates/dds4ccm/src/dlrl.rs::CacheOperation`: die sechs Attribute als Getter, die sieben Operationen, jede mit der Signatur der gleichnamigen Methode von `zerodds_dlrl::cache::Cache`; `Cache` implementiert den Trait mit diesen Methoden. Die Exceptions sind die Varianten von `DlrlException`: `refresh` erhebt `DCPSError`, `create_access` und `delete_access` `PreconditionNotMet`; `AlreadyDeleted` steht in keiner `raises`-Klausel und kommt aus jeder Operation, sobald der Cache gelöscht ist (DDS 1.2 §8.2.1.1), etwa nach `remove` des Connectors. Das Receptacle: `crates/dds4ccm/src/dlrl.rs::DlrlPort::cache`. Das frühere Datenmodell (`DlrlPort`, `DlrlPortKind` in `crates/ccm/src/dds4ccm.rs`) ist entfernt.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_1_1_cache_operation_is_the_operative_part_of_the_cache_of_the_port`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131. Geprüft über zwei Container: die Attribute sind die des Cache des Ports (`DlrlConnector::cache_of`); `refresh` wendet im manuellen Modus die angekommenen Updates an, ohne Listener; `attach_listener` und `detach_listener` wirken auf den Cache; `disable_updates` und `enable_updates` rufen `on_updates_disabled` und `on_updates_enabled`, danach kommt ein Update mit einer Runde (`on_begin_updates`, `on_end_updates`); `create_access` und `delete_access` wirken auf `sub_accesses`, `PreconditionNotMet` für einen schreibenden Zweck auf einem READ_ONLY-Cache und für den CacheAccess eines anderen Cache; nach `remove` des Connectors erhebt jede Operation `AlreadyDeleted`, die Attribute behalten ihren Wert. Die IDL von Annex B und ihr Abgleich mit dem Trait: siehe Annex B.

#### §8.2.1.2 DLRL Class (ObjectHome)

**Spec:** §8.2.1.2, S. 30 (PDF) — je DLRL-Klasse ein Receptacle vom Typ der Home, die von `DDS::ObjectHome` erbt und vom DLRL-Werkzeug erzeugt ist; "All accesses to the DLRL objects of this type will be manageable through this entry point."

**Repo:** `crates/dds4ccm/src/dlrl.rs::{HomeReceptacle, DlrlPortType, DlrlPort::home}`: ein Home-Receptacle je Klasse des Objektmodells, deklariert mit dem Namen der Klasse; `DlrlPort::home` gibt zum Namen des Receptacles die `zerodds_dlrl::cache::ObjectHome` der Klasse. Das Objektmodell mit Mapping erzeugt der DLRL-Generator von `crates/dlrl` (`generation::generate`, DDS 1.2 §8.2.2.1) aus Modellbeschreibung und Model-Tags; `DlrlPortType::new` nimmt nur Klassen dieses Modells. Das Fragment erzeugt je Home-Receptacle die Home seiner Klasse und registriert sie beim Cache des Ports, in der Reihenfolge der Receptacles. Einen Typ je Home, der von `ObjectHome` erbt und vom DLRL-Werkzeug erzeugt ist (`FooHome`), gibt es in Rust nicht: das Receptacle ist der generische Typ `ObjectHome`, ausgewählt über den Namen der Klasse. Das implizite IDL (`zerodds-idlc dlrl --lang idl`) deklariert `FooHome` über `DDS::ObjectHome`, `crates/dlrl-codegen` schreibt Home-Klassen für C++, Java, C# und TypeScript.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_2_1_2_each_home_receptacle_is_the_home_of_its_class_of_the_generated_model, dds4ccm_8_2_2_a_port_type_has_one_cache_operation_receptacle_and_a_home_receptacle_per_class}`.

**Status:** partial — gebaut und auf dem Linux-Testhost grün (`cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131): je Klasse ein Home-Receptacle, das die beim Cache des Ports registrierte Home dieser Klasse ist. Offen: das Receptacle hat nicht den Typ einer erzeugten Home-Klasse, die von `DDS::ObjectHome` erbt, sondern den generischen Typ `ObjectHome`. Es fehlen die für Rust erzeugten Klassen nach DDS 1.2 §8.1.6.6 (`FooHome` mit `create_object(access)` und den übrigen typisierten Operationen, dazu `Foo`, `FooListener`, `FooSelection`, `FooSelectionListener`, `FooFilter`, `FooQuery`); sie sind in `dlrl-1.2` §8.2.2, Punkt (3), offen. Mit ihnen wird das Receptacle vom Typ `FooHome`. Das Modell des Tests erzeugt der DLRL-Generator aus Modellbeschreibung und Model-Tags. Geprüft: jedes Home-Receptacle ist die beim Cache des Ports registrierte Home seiner Klasse (`find_home_by_name`, `registration_index`) mit dem Mapping des Port-Typs; über sie werden Objekte der Klasse erzeugt, gefunden (`get_objects`, `find_object`), ihre Topics benannt, und ein Objekt, das im Cache ankommt, meldet ihr `ObjectListener`; ein Name, den der Port-Typ nicht hat, gibt keine Home; eine Klasse, die das Modell nicht hat, weist `DlrlPortType::new` ab (`UnknownClass`).

#### §8.2.2 DLRL Extended Ports Composition Rule

**Spec:** §8.2.2, S. 31 (PDF) — ein DLRL-Port besteht aus einem `CacheOperation`-Receptacle und einem Receptacle je `ObjectHome`; das Fragment erzeugt den Cache nach `cache_usage`, registriert die Homes, wendet das QoS-Profil an und schaltet die Infrastruktur ein.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlPortType::new` (ein `CacheOperation`-Receptacle, mindestens ein Home-Receptacle, je Klasse eines, mit dem generischen Typ `ObjectHome`, siehe §8.2.1.2; die Namen sind IDL-Bezeichner, kollidieren mit keinem IDL-Schlüsselwort, auch nicht ohne Groß-/Kleinschreibung (IDL 4.2 §7.2.4, die Tabelle `KEYWORDS` von `crates/idl-cst`), und sind auch ohne Groß-/Kleinschreibung verschieden), `crates/dds4ccm/src/dlrl.rs::DlrlConnector::{connect, complete}`: (1) der Cache für `cache_usage` (ein readonly-Attribut der Schnittstelle; das Fragment bekommt es wie der Mirror-Port `observable` das readonly-Attribut `is_global_scope`); (2) je Home-Receptacle die Home, `register_home`, `register_all_for_pubsub`; (3) das QoS-Profil: das `topic_qos` jedes Topics vor `register_all_for_pubsub` als QoS eines Topics, das der Cache erzeugt (`crates/dlrl/src/cache/mod.rs::Cache::set_new_topic_qos`; ein Topic ist ab dem Erzeugen aktiviert); ein Topic, das der DomainParticipant beim Erzeugen der Topics des Cache schon hat (eines anderen Ports, auch eines gleichzeitig verbundenen, oder der Anwendung), behält seine QoS und dient dem Port, wenn jede Policy, die das `topic_qos` des Profils setzt, dort denselben Wert hat (sonst `ConnectorError::TopicQos`, wie bei den DCPS-Connectoren); danach `publisher_qos`, `subscriber_qos`, `datawriter_qos` und `datareader_qos` über der QoS, die der Cache den noch nicht aktivierten Entities gab (`crates/dds4ccm/src/entities.rs::Profile::{publisher_qos_over, subscriber_qos_over, datawriter_qos_over, datareader_qos_over}`); eine Policy, die das Profil nicht setzt, behält den Wert des Cache; `topic_filter` wählt die Container eines Topics nach seinem Namen im Mapping, DomainParticipant, Publisher und Subscriber nehmen den Container ihrer Art, der für jedes Topic des Ports gewählt ist (siehe §8.4.2); (4) `enable_all_for_pubsub`. Ein abgewiesenes Fragment löscht, was es erzeugt hat.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other, dds4ccm_8_2_2_a_port_type_has_one_cache_operation_receptacle_and_a_home_receptacle_per_class, dds4ccm_8_2_2_a_refused_fragment_leaves_no_entity, dds4ccm_8_2_2_ports_connected_at_the_same_time_are_judged_by_the_topics_they_meet, dds4ccm_8_4_2_topic_filter_assigns_the_values_of_the_profile_to_the_entities_of_each_topic}`, `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_3_the_qos_of_a_topic_is_given_before_register_all_for_pubsub, dlrl_8_1_5_3_a_qos_for_a_new_topic_leaves_a_topic_that_exists_as_it_is}`.

**Status:** partial — gebaut und auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 132 bestanden, 2 übersprungen (die beiden omniidl-Tests mit `#[ignore]`, Annex B); `-p zerodds-dlrl --all-targets` 140 bestanden, 1 übersprungen. Offen: der zweite Punkt der Regel, "As many DDS:ObjectHome-derived receptacles as DLRL object types", ist mit Receptacles vom generischen Typ `ObjectHome` gebaut; die vom DLRL-Werkzeug für Rust erzeugten Home-Klassen fehlen (siehe §8.2.1.2, `dlrl-1.2` §8.2.2, Punkt (3)). Geprüft: zwei Container mit je einem Connector; der Port des einen (WRITE_ONLY) schreibt einen Radar und einen Track mit Relation auf den Radar, bevor der Port des anderen (READ_ONLY) besteht; beide kommen dort als Objekte mit ihren Werten, ihrer Relation und ihrer Home an, über DDS. Der Cache hat die Nutzung des Ports, die Homes in der Reihenfolge der Receptacles, den DCPS-Zustand ENABLED; die Werte des Profils (TRANSIENT_LOCAL für Topic, DataWriter und DataReader, KEEP_ALL für die DataWriter, Partition für Publisher und Subscriber) sind an den Entities, die DataReader bleiben RELIABLE, wo das Profil schweigt. Die Deklaration weist ab: kein Home-Receptacle, zwei Receptacles einer Klasse, eine fremde Klasse, doppelte Namen (auch nur in der Schreibweise verschieden), Namen, die keine Bezeichner sind, und Namen, die mit einem IDL-Schlüsselwort kollidieren (`home`, `Uses`, `PortType`, `LOCAL`); ein Name, der ein Schlüsselwort nur enthält (`HomePort`), ist erlaubt. Die Zuteilung des Profils über `topic_filter`: siehe §8.4.2. Eine Klasse, deren Relationsziel kein Home-Receptacle hat, weist der Cache ab (`BadHomeDefinition`); ein vorhandenes TRANSIENT_LOCAL-Topic dient einem Profil ohne `topic_qos` und weist eines mit VOLATILE ab (`TopicQos`); ein abgewiesenes Fragment lässt keine Entity im DomainParticipant. Ein Port mit TRANSIENT_LOCAL-Topics im Profil und einer ohne `topic_qos` werden in 40 Runden gleichzeitig im selben DomainParticipant verbunden: der zweite wird in jeder Runde angenommen; der erste wird angenommen, mit TRANSIENT_LOCAL-Topics, oder mit `TopicQos` für ein Topic abgewiesen, das der zweite VOLATILE erzeugt hat, nie mit einem Fehler der DLRL-Entities.

### §8.3 DDS-DLRL Connectors

**Spec:** §8.3, S. 31 (PDF) — ein DLRL-Connector erbt von `DDS_Base` (`error_listener`, `domain_id`, `qos_profile`) und hat einen Mirror-Port je DLRL-Port.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector`: dereferenziert auf `DdsBase` (`error_listener`, `domain_id`, `qos_profile`, `configuration_complete`, `NonChangeable` danach); `new` mit den Mirror-Ports (mindestens einer, verschiedene Bezeichner), `mirror_ports`, `connect` mit dem Namen des Mirror-Ports (`ConnectorError::UnknownMirrorPort`, `NotConfigured` vor `configuration_complete`), `cache_of` (der Cache eines Ports für die Konfiguration), `remove` (löscht die Caches mit ihren Entities und die DomainParticipants, die einem Port gewidmet sind; danach erhebt das `CacheOperation` eines Ports `AlreadyDeleted`). `error_listener` bekommt die Status der DCPS-Entities der Ports: `crates/dds4ccm/src/status.rs::WriterStatusListener` an jedem DataWriter, `SubscriberStatusListener` am Subscriber des Cache (die Status der DataReader außer DATA_AVAILABLE, das der Cache nimmt, steigen zum Subscriber auf; REQUESTED_DEADLINE_MISSED und SAMPLE_LOST mit `on_unexpected_status`, ein DLRL-Port hat keinen `PortStatusListener`), `crates/dds4ccm/src/connector.rs::share_topic_listener` (der `TopicListener` eines Topics, geteilt mit den anderen Connectoren des Topics im DomainParticipant; ein Connector mit zwei Ports dort wird einmal gerufen).

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_3_a_dlrl_connector_is_a_dds_base_with_a_mirror_port_per_dlrl_port, dds4ccm_8_3_error_listener_gets_the_statuses_of_the_dcps_entities_of_the_ports, dds4ccm_8_3_two_ports_of_a_connector_in_one_participant_hear_inconsistent_topic_once}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131. Geprüft: die Deklaration weist keinen Mirror-Port, doppelte und ungültige Namen ab, auch einen, der mit einem IDL-Schlüsselwort kollidiert (`MirrorPort`); die Attribute von `DDS_Base` sind bis `configuration_complete` setzbar, danach `NonChangeable`; `connect` vorher gibt `NotConfigured`, ein unbekannter Mirror-Port `UnknownMirrorPort`; zwei Mirror-Ports zweier Port-Typen, die das Topic `Radar` teilen, tauschen ein Objekt aus; der Container behält seine Defaults, solange ein Port sie nutzt (`PRECONDITION_NOT_MET`), nach `remove` des Connectors lässt er sich entfernen. `error_listener` bekommt PUBLICATION_MATCHED und SUBSCRIPTION_MATCHED (`on_unexpected_status`), OFFERED_INCOMPATIBLE_QOS vom DataWriter eines Ports und REQUESTED_INCOMPATIBLE_QOS von seinem DataReader gegenüber Entities eines anderen Teilnehmers, INCONSISTENT_TOPIC; der Cache wendet die Updates weiter an. Zwei Ports eines Connectors im selben DomainParticipant und ein Port eines zweiten Connectors dort: ein INCONSISTENT_TOPIC erreicht jeden `error_listener` genau einmal (ohne die Prüfung in `TopicStatusListener` kommt es beim ersten zweimal an, auf dem Linux-Testhost so beobachtet).

### §8.4 Configuration and QoS Support

#### §8.4.1 DDS Entities

**Spec:** §8.4.1, S. 31-32 (PDF) — ein DLRL-Port hat einen eigenen Publisher und/oder Subscriber (nach `cache_usage`) und je Topic der DLRL-Objekte einen DataReader und/oder DataWriter.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector::connect` über `zerodds_dlrl::cache::{CacheFactory::create_cache, Cache::register_all_for_pubsub}` (ein Publisher bei `WRITE_ONLY` und `READ_WRITE`, ein Subscriber bei `READ_ONLY` und `READ_WRITE`, je Topic der Klassen des Ports ein DataWriter und ein DataReader darin); der DomainParticipant nach §7.4.1 und §7.4.3: `crates/dds4ccm/src/entities.rs::Component::place_participant` (der Default des Containers, sonst der der Komponente, wenn er hat, was das `domainparticipant_qos` des Profils setzt, sonst einer, der dem Port gewidmet ist). Die Default-Publisher und -Subscriber von Container und Komponente dienen keinem DLRL-Port.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_4_1_a_dlrl_port_has_its_own_publisher_and_subscriber_and_an_entity_per_topic`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131. Geprüft am DomainParticipant des Containers mit gegebenem Default-Publisher und -Subscriber: ein READ_WRITE-Port fügt einen Publisher, einen Subscriber und je Topic einen DataWriter und einen DataReader hinzu, ein READ_ONLY-Port nur einen Subscriber und die DataReader, ein WRITE_ONLY-Port nur einen Publisher und die DataWriter; jeder Port hat eigene, die Defaults bleiben ohne Endpunkt; die Topics gibt es einmal; ein Profil mit einem `domainparticipant_qos`, das der Default nicht hat, bekommt einen gewidmeten DomainParticipant mit dieser QoS; nach `remove` sind Publisher, Subscriber und Topics der Ports wieder weg.

#### §8.4.2 Use of QoS Profiles

**Spec:** §8.4.2, S. 32 (PDF) — wie §7.4.2 und §7.4.3; das Profil des DLRL-Connectors enthält die Werte aller Topics, `topic_filter` teilt sie zu.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector::{connect, complete}` mit dem Leser der QoS-Profile der DCPS-Connectoren (`crates/dds4ccm/src/entities.rs::Profile`, §7.4.2, §7.4.3): das Profil des Connectors gilt für alle seine Ports und Topics; `topic_filter` wählt für jedes Topic des Ports nach seinem Namen im Mapping das `topic_qos` (vor `register_all_for_pubsub`), das `datawriter_qos` und das `datareader_qos` (vor `enable_all_for_pubsub`). DomainParticipant, Publisher und Subscriber dienen allen Topics des Ports: jeder nimmt den Container seiner Art, der für jedes Topic gewählt ist (ein Container ohne `topic_filter` oder einer, dessen Filter alle Topics trifft), keinen, wenn für kein Topic einer gewählt ist (`Profile::{participant_of_topics, publisher_qos_over, subscriber_qos_over}`). Wählt das Profil für zwei Topics des Ports verschiedene Container einer dieser Arten, weist der Connector den Port mit `ConnectorError::TopicFilter` ab; das Profil sagt nicht, welchen davon die eine Entity nimmt. Ein Container, dessen Filter kein Topic des Ports trifft, gilt für ihn nicht.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_4_2_topic_filter_assigns_the_values_of_the_profile_to_the_entities_of_each_topic, dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131. Geprüft mit einem Profil für die Topics `Radar` und `Track`: TRANSIENT_LOCAL nur am Topic `Track`; KEEP_ALL am DataWriter von `Track`, KEEP_LAST 3 aus dem Container ohne Filter an dem von `Radar`; KEEP_LAST 5 am DataReader von `Radar` (`topic_filter="Rad*"`), der DataReader von `Track` behält KEEP_LAST 1, beide RELIABLE vom Cache; die Partition des `publisher_qos` ohne Filter am Publisher, die des `subscriber_qos` mit `topic_filter="*"` am Subscriber. Ein `publisher_qos` nur für `Track` gilt nicht für den Publisher eines Ports nur mit `Radar`; einen WRITE_ONLY-Port mit `Radar` und `Track` weist der Connector mit `TopicFilter` ab, ohne eine Entity zu hinterlassen, einen READ_ONLY-Port ohne Publisher nicht. Ein `domainparticipant_qos` nur für `Track`: ein Port nur mit `Radar` bekommt den Default-DomainParticipant des Containers, ein Port mit beiden Topics `TopicFilter`.

---

## Annex A — IDL3+ of DDS-DCPS Ports and Connectors

**Spec:** Annex A, S. 33-40 (PDF, normativ) — die IDL3+ des Moduls `CCM_DDS`:
der Teil ohne Datentyp (Typen, Konstante `UNLIMITED`, Exceptions, Interfaces,
die Connector-Basen `DDS_Base` und `DDS_TopicBase`) und das Template-Modul
`Typed<typename T, sequence<T> TSeq>` mit den Interfaces, Port-Typen und
Connectoren für einen Datentyp; `#include "dds_rtf2_dcps.idl"`.

**Repo:** `crates/dds4ccm/idl/ccm_dds.idl` (der Text des Anhangs; ein Token
korrigiert: `DDS::Duration_ttime_out` ist `DDS::Duration_t time_out`),
`crates/dds4ccm/idl/dds_rtf2_dcps.idl` (die Typen des Moduls `DDS`, auf die der
Anhang verweist); die Rust-Abbildung `crates/dds4ccm/src/ccm_dds.rs`,
`crates/dds4ccm/src/ccm_dds_typed.rs`; im IDL-Frontend
`crates/idl-lower/src/lower.rs`, `crates/idl-lower/src/scope.rs` (der Rumpf
eines Template-Moduls sieht den Scope seiner Deklaration; ein
Sequenz-Parameter über einen früheren Parameter, idl-4.2 §7.4.12.4).

**Tests:** `crates/dds4ccm/tests/annex_idl.rs::{dds4ccm_annex_a_the_non_typed_part_of_ccm_dds_compiles_to_the_declarations_of_the_annex, dds4ccm_annex_a_instantiating_typed_gives_the_interfaces_for_a_data_type, dds4ccm_annex_a_the_port_types_and_connectors_of_an_instance_are_those_of_the_annex, dds4ccm_annex_a_a_port_gives_a_component_its_basic_ports_and_a_connector_their_mirror, dds4ccm_annex_a_the_rust_interfaces_have_the_operations_of_the_idl}`, `crates/dds4ccm/src/ccm_dds.rs::{tests::dds4ccm_annex_a_unlimited_is_zero_and_the_defaults_are_those_of_the_idl, tests::dds4ccm_annex_a_internal_error_carries_the_dds_return_code_and_the_index}`, `crates/idl-lower/tests/idl_4_2_rules.rs::{idl_7_4_12_4_2_a_template_body_names_what_the_scope_of_its_declaration_declares, idl_7_4_12_4_1_a_sequence_formal_over_an_earlier_formal_is_the_sequence_passed_for_it, idl_7_4_12_4_2_an_actual_that_is_not_the_sequence_of_its_formal_is_refused, idl_7_4_12_4_2_a_sequence_formal_over_an_anonymous_element_takes_the_compliant_sequence}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113, `-p zerodds-idl-lower -p zerodds-idl-compose -p zerodds-ccm -p zerodds-idl-parse --all-targets` 726 von 726. Die Datei läuft durch `idl-compose` (der `#include`), `idl-parse` und `idl-lower`. Geprüft wird: jeder Typ, jede Konstante, jede Exception, jedes Interface und jede Connector-Basis des Teils ohne Datentyp mit ihren Membern, Operationen, Parametern und `raises`; `module CCM_DDS::Typed<Foo, FooSeq> Foo_conn;` gibt die sieben Interfaces mit `Foo` und `FooSeq` an Stelle von `T` und `TSeq`, die sechs Port-Typen und die Connectoren `DDS_State` und `DDS_Event` mit den Ports und Attributen des Anhangs; eine Komponente mit einem Port eines solchen Port-Typs bekommt dessen Basic Ports, `DDS_State` deren Spiegel (Equivalent-IDL von `crates/ccm`); die Traits von `ccm_dds` und `ccm_dds_typed` haben die Operationen und Attribute der IDL und keine anderen, jede mit den Parametern der IDL (Name, Reihenfolge, Richtung: `in` als Wert oder `&`, `inout` und `out` als `&mut`; Typ nach der Abbildung der Moduldokumentation), dem Ergebnistyp und den Exceptions (eine als ihre Struktur, mehrere als `UpdaterError` oder `ReaderError`, deren Varianten die Exceptions der Operationen des Interfaces sind), ein Attribut als Getter und, außer `readonly`, als Setter mit seinem `setraises`, das Datenmodell von `crates/ccm` nennt dieselben Port-Typen, Basic Ports, Connectoren und Attribute. Die Rust-Traits sind von Hand geschrieben und werden gegen die IDL geprüft, nicht aus ihr erzeugt. Nicht geprüft: die Datei mit einem IDL-Compiler eines anderen Herstellers. omniidl aus omniORB 4.3.3 ist auf dem Linux-Testhost installiert und weist die IDL3+ des Anhangs ab: `omniidl -bcxx ccm_dds.idl` endet mit `ccm_dds.idl:143: Syntax error in definition` (Zeile 143 ist `connector DDS_Base {`) und weiteren Syntaxfehlern in den Zeilen 144 bis 148. `tao_idl` ist auf dem Linux-Testhost nicht installiert.

---

## Annex B — IDL for DDS-DLRL Ports and Connectors

**Spec:** Annex B, S. 41 (PDF, normativ) — `local interface CacheOperation`
im Modul `CCM_DDS`, `#include "dds_rtf2_dlrl.idl"`.

**Repo:** `crates/dds4ccm/idl/ccm_dds_dlrl.idl` (der Text des Anhangs);
`crates/dds4ccm/idl/dds_rtf2_dlrl.idl` (bindet die generische DLRL-IDL von
DDS 1.2 §8.2.1.2.1 ein, `crates/dlrl/idl/dds_dlrl.idl`, die auf dem
Include-Pfad liegen muss wie das IDL-Verzeichnis einer DDS-Installation;
davor `dds_rtf2_dcps.idl`, und der Guard `DDS_DCPS_IDL` macht dessen
Deklarationen von `StringSeq`, `Entity`, `DomainParticipant`, `Publisher`
und `Subscriber` zu den einzigen, so dass Annex A und Annex B in einer
Einheit übersetzen); `crates/dds4ccm/idl/dds_rtf2_dcps.idl` (dazu
`DomainParticipant`, `Publisher` und `Subscriber`, auf die `DDS::Cache` und
`DDS::CacheDescription` verweisen); die Rust-Abbildung
`crates/dds4ccm/src/dlrl.rs::CacheOperation` (die Abbildung steht in der
Dokumentation des Traits); `ci/integration_scenarios.py` (Job
`integration-bridges-vendor-corba`, Szenario `dds4ccm-idl-omniidl`).

**Tests:** `crates/dds4ccm/tests/annex_idl.rs::{dds4ccm_annex_b_cache_operation_compiles_to_the_local_interface_of_the_annex, dds4ccm_annex_b_cache_operation_retains_attributes_and_operations_of_dds_cache_unchanged, dds4ccm_annex_b_compiles_with_annex_a_in_one_unit, dds4ccm_annex_b_the_rust_cache_operation_has_the_operations_of_the_idl}`, `crates/dds4ccm/tests/omniidl.rs::{dds4ccm_annex_b_omniidl_compiles_the_idl_of_the_annex, dds4ccm_annex_b_omniidl_refuses_the_annex_without_the_dlrl_idl}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 von 131; `cargo nextest run --profile merge -p zerodds-dds4ccm --test omniidl --run-ignored only` 2 von 2 mit omniidl aus omniORB 4.3.3 (Debian-Paket `omniidl`). Die Datei läuft durch `idl-compose` (die `#include`), `idl-parse` und `idl-lower`. Geprüft wird: `CCM_DDS::CacheOperation` ist ein `local interface` ohne Basis mit den sechs `readonly`-Attributen und den sieben Operationen des Anhangs, jede mit Parametern (Richtung, Typ, Name), Ergebnis und `raises`; die Typen sind die der generischen DLRL-IDL (`DDS::CacheUsage` mit `READ_ONLY`, `WRITE_ONLY`, `READ_WRITE`, die vier Sequenzen über `ObjectRoot`, `ObjectHome`, `CacheAccess`, `CacheListener`, die Exceptions mit `string message`). "All the retained functions mimic exactly the DDS::Cache ones" (§8.2.1.1): jedes Attribut und jede Operation von `CacheOperation` ist in `DDS::Cache` oder seiner Basis `DDS::CacheBase` genau einmal und gleich deklariert; weggelassen sind die Einrichtung des Cache, die das Fragment übernimmt (`register_home`, `register_all_for_pubsub`, `enable_all_for_pubsub`, `pubsub_state`, `the_publisher`, `the_subscriber`, `find_home_by_name`, `find_home_by_index`), `kind`, `load`, `lock` und `unlock`. Annex A und Annex B übersetzen zusammen in beiden Reihenfolgen; ohne den Guard kollidieren die zwei Deklarationen von `DDS::StringSeq` und `DDS::Entity`, ohne die DLRL-IDL auf dem Include-Pfad übersetzt Annex B nicht. Der Trait `CacheOperation` hat die Attribute und Operationen der IDL und keine anderen, mit den Parametern der IDL (Name, Reihenfolge, Typ nach der Abbildung) und dem Ergebnis; jede Methode hat die Signatur der gleichnamigen Methode von `zerodds_dlrl::cache::Cache`; `DCPSError`, `PreconditionNotMet` und `AlreadyDeleted` sind Varianten von `DlrlException` mit der Repository-Id der Exception der DLRL-IDL. omniidl übersetzt die Datei mit seinem C++-Backend; die Klasse `CacheOperation` hat jedes Attribut und jede Operation, ohne die DLRL-IDL auf dem Include-Pfad weist omniidl die Datei ab. §8.2.1.1 druckt `void DDS::refresh( )`, Annex B `void refresh( )`; die Datei folgt dem Anhang, ein Operationsname ist ein Bezeichner (IDL 4.2 §7.4.3.4.3.3.1, Regel 82). Das frühere Datenmodell (`DlrlPort`, `ConnectorPattern::Dlrl` in `crates/ccm/src/dds4ccm.rs`) ist entfernt.

---

## Annex C — XML Schema for QoS Profiles

**Spec:** Annex C, S. 43-50 (PDF, normativ) — das XML-Schema der QoS-Profile:
Namensraum `http://www.omg.org/dds/`, die einfachen Typen, je Policy ein
komplexer Typ, die sechs QoS-Container, `qosProfile` mit `name`, `base_name`
und `topic_filter`.

**Repo:** `crates/xml/schemas/dds4ccm_qos_profile.xsd` (die Typdefinitionen
des Anhangs; ergänzt um das globale Element `dds`, die Wurzel von §7.4.2.1,
denn ein Schema ohne globales Element hat kein gültiges Dokument),
`crates/xml/src/schemas.rs::{DDS4CCM_QOS_PROFILE_XSD, DDS4CCM_QOS_PROFILE_NAMESPACE}`;
der Leser `crates/xml/src/qos_parser.rs`.

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_c_the_schema_file_has_the_types_of_the_annex_and_the_root_dds, dds4ccm_annex_c_what_the_schema_accepts_we_read, dds4ccm_annex_c_what_the_schema_rejects_we_reject, dds4ccm_7_4_2_the_forms_of_the_text_the_schema_rejects_have_their_values, dds4ccm_7_4_2_2_2_octets_are_the_list_of_the_text_or_the_base64_of_the_schema}`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/dds4ccm/tests/pattern_profiles.rs::dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548; `xmllint` (libxml2-utils) ist der Vergleich. Was das Schema annimmt, liest der Leser, mit dem Wert jedes Feldes. Was es abweist, weist der Leser ab, außer an den Stellen, an denen der Text von §7.4.2 anderes schreibt; jede steht mit ihrer Klausel im Test (siehe §7.4.2.2 und §7.4.2.3): Elemente ohne Namensraum, `topic_filter` mit `*`, Namen mit `_` und `:`, `DURATION_ZERO_SEC`/`DURATION_ZERO_NSEC`, eine hexadezimale `history/depth` und `resource_limits/max_samples`, die Booleans `yes` und `TRUE`, ein negativer `transport_priority/value`, `<participant_qos>`, Oktette als Liste, `<qos_library>` (die Form von DDS-XML 1.0). Ebenso nach §7.4.2.1 Table 7.1: die Booleans `yes` und `TRUE`, eine hexadezimale Zahl in `resource_limits`, ein negativer `transport_priority/value`. Die sechs Typen `…QosProfile` schränken ihren Basistyp ohne Inhaltsmodell ein und haben damit nach XML Schema Part 1, 3.4.2, leeren Inhalt; das Element `dds` deklariert das einzelne QoS von §7.4.2.3.3 deshalb mit dem Basistyp (Kopf der Datei). In CI kehren die Vergleiche mit `xmllint` ungeprüft zurück (`[skip]`), bis das Image `ci-rust` aus `ci/Dockerfile.rust` neu gebaut ist.

---

## Annex D — Default QoS Profile

**Spec:** Annex D, S. 51-55 (PDF, nicht normativ) — ein QoS-Profil "with all
default values as specified in DDS", mit jeder Policy der sechs Container.

**Repo:** `crates/xml/schemas/dds4ccm_default_qos_profile.xml` (das Dokument
des Anhangs), `crates/xml/src/schemas.rs::DDS4CCM_DEFAULT_QOS_PROFILE_XML`.

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_d_the_default_profile_is_valid_against_the_schema_of_annex_c, dds4ccm_annex_d_the_default_profile_reads_to_the_dds_defaults}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 von 548. Das Dokument ist gültig gegen das Schema von Annex C (`xmllint`); jeder Wert, den der Leser daraus macht, ist der Default von `zerodds_qos`. Der Name des Profils ist `" DDS DefaultQosProfile"`, mit dem führenden Leerzeichen des Anhangs. In CI kehren die Vergleiche mit `xmllint` ungeprüft zurück (`[skip]`), bis das Image `ci-rust` aus `ci/Dockerfile.rust` neu gebaut ist.

---

## Annex E — QoS Policies for the DDS Patterns

**Spec:** Annex E, S. 57-61 (PDF, nicht normativ) — vier Tabellen, je Rolle
(Observable und Observer des State-Patterns, Supplier und Consumer des
Event-Patterns) die Policies von Topic, DataReader, DataWriter, Subscriber und
Publisher, mit der Angabe, ob das Pattern den Wert verlangt.

**Repo:** `crates/dds4ccm/src/profiles.rs::{STATE_TRANSFER, EVENT_TRANSFER}`,
`crates/dds4ccm/profiles/state_transfer.xml`,
`crates/dds4ccm/profiles/event_transfer.xml`.

**Tests:** `crates/dds4ccm/tests/pattern_profiles.rs::{dds4ccm_annex_e_the_state_profile_has_the_values_of_the_observable_and_the_observer_table, dds4ccm_annex_e_the_event_profile_has_the_values_of_the_supplier_and_the_consumer_table, dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c, dds4ccm_annex_e_a_state_connector_with_the_state_profile_gives_a_late_observer_the_last_value, dds4ccm_annex_e_an_event_connector_with_the_event_profile_delivers_what_is_sent_from_then_on}`.

**Status:** done — auf dem Linux-Testhost grün: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 von 113. Ein Connector hat ein `qos_profile` für alle Ports; die zwei Tabellen eines Patterns sind deshalb ein Profil: DataWriter und Publisher der schreibenden Rolle, DataReader und Subscriber der lesenden, das Topic, für das beide Tabellen dieselben Werte geben. State: `TRANSIENT_LOCAL`, `RELIABLE`, `KEEP_LAST` mit Tiefe 1, `BY_SOURCE_TIMESTAMP`, PRESENTATION `INSTANCE` mit `ordered_access`. Event: `VOLATILE`, `BEST_EFFORT`, `KEEP_ALL`, `BY_SOURCE_TIMESTAMP`, PRESENTATION `INSTANCE` mit `ordered_access`, `autodispose_unregistered_instances` FALSE. Die Tests lesen die Werte am Profil ab und über DDS: ein spät hinzukommender Observer eines `DDS_State` mit dem State-Profil bekommt den letzten Wert, ein `DDS_Event` mit dem Event-Profil liefert, was ab dann gesendet wird. Festlegung: der Connector wendet die Profile nicht von sich aus an; ohne `qos_profile` haben seine Entities die Default-QoS von DDS und die HISTORY seines Patterns (§7.3.2, §7.3.3). Grund: PRESENTATION und DESTINATION_ORDER werden zwischen DataWriter und DataReader verglichen, und ein Default-Publisher oder -Subscriber eines Containers passt nur mit gleicher PRESENTATION (§7.4.3); ein Connector mit diesen Werten als Default würde mit einer DDS-Anwendung mit Default-QoS nur in einer Richtung zusammenpassen. In CI kehrt der Vergleich mit `xmllint` (`dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c`) ungeprüft zurück (`[skip]`), bis das Image `ci-rust` aus `ci/Dockerfile.rust` neu gebaut ist.

---

## Audit-Status

21 done / 4 partial / 0 open / 0 blocked / 10 n/a (informative).

Test-Lauf: die Erhebung aus CI (`python3 ci/coverage_results.py --document dds4ccm-1.1.md`, Pipelines 8501 bis 9053 von `main`) ist älter als die Tests, die dieses Dokument jetzt zitiert (`crates/dds4ccm`, `crates/idl-lower`, `crates/xml/tests/dds4ccm_profiles.rs`); ihre Zahlen gelten nicht mehr und stehen deshalb nicht hier. Sie wird nach dem Merge neu erhoben. Bis dahin stehen die Ergebnisse vom Linux-Testhost in den Status-Zeilen der Items.

Offene Punkte: `dds4ccm-1.1.open.md`.
