# DDS for Lightweight CCM 1.1 — Spec Coverage

**Spec:** [OMG DDS for Lightweight CCM 1.1 — formal/2012-02-01 (~95 pages) →](https://www.omg.org/spec/DDS4CCM/)

**Context:** DDS4CCM defines how CCM components interact with DDS: DDS-DCPS
extended ports + connectors (§7) for pub/sub usage, plus DDS-DLRL extended
ports + connectors (§8) for the optional object-layer usage. The XML-QoS
profile definition (§7.4.2 + Annex C XSD + Annex D default profile) is the
central QoS configuration spec.

The implementation is spread across several crates:

- `crates/ccm/` — the data model of the DDS-DCPS ports and the connectors: their names, the basic ports of a port type, the mirror ports of a connector, the levels of the threading policy (`src/dds4ccm.rs`)
- `crates/dds4ccm/` — the connector fragments of the DDS-DCPS ports (§7.2.2), the connectors (§7.3), their DCPS entities and QoS profiles (§7.4.1, §7.4.3) and the threading policy (§7.4.4) over the DCPS entities; the IDL of Annex A (`idl/ccm_dds.idl`); the QoS profiles of the two patterns (Annex E, `profiles/`); the DLRL ports and connectors (§8, `src/dlrl.rs`) over the DLRL entities of `crates/dlrl/`; the IDL of Annex B (`idl/ccm_dds_dlrl.idl`)
- `crates/xml/` — the reader of the XML QoS profiles (§7.4.2), the schema of Annex C (`schemas/dds4ccm_qos_profile.xsd`) and the default profile of Annex D (`schemas/dds4ccm_default_qos_profile.xml`)
- `crates/dcps/` — the DCPS entities under the connector fragments
- `crates/idl-parse/` + `crates/idl-lower/` + `crates/idl-compose/` — read the IDL of Annex A (template modules, `porttype`, `connector`); `crates/ccm/` maps ports and connectors to equivalent IDL
- `crates/corba-ccm/` — CCM container stack (see `omg-ccm-4.0.md`). It has no dependency on `crates/ccm/` and `crates/dds4ccm/`: the fragments of §7 run in the `Container` of `crates/dds4ccm/src/entities.rs`
- `crates/dlrl/` — has `Cache`, `CacheAccess`, `CacheUsage`, `ObjectHome` and `CacheListener` of DDS-DLRL (`crates/dlrl/src/cache/`) and their IDL (`crates/dlrl/idl/dds_dlrl.idl`); the DLRL ports and connectors of §8 on them are in `crates/dds4ccm/src/dlrl.rs`; the IDL of Annex B includes `dds_dlrl.idl` through `crates/dds4ccm/idl/dds_rtf2_dlrl.idl`

**Crate mapping:**

| Spec area | Crate / module |
|---|---|
| §2 Conformance, point 1 | `crates/dds4ccm/` (test `tests/conformance.rs`) |
| §7.2.2 Normative DDS-DCPS ports | `crates/dds4ccm/` |
| §7.3 DDS-DCPS connectors | `crates/dds4ccm/src/connector.rs` |
| §7.4.1 DCPS entities, §7.4.3 Use of QoS profiles | `crates/dds4ccm/src/entities.rs` |
| §7.4.4 Threading policy | `crates/dds4ccm/src/threading.rs` |
| §7 Data model: names of the DDS-DCPS ports and of the connectors | `crates/ccm/src/dds4ccm.rs` |
| §7.4.2 DDS QoS policies in XML | `crates/xml/src/qos_parser.rs`, `crates/xml/src/qos.rs` |
| §8 DDS-DLRL extended ports + connectors | `crates/dds4ccm/src/dlrl.rs` |
| Annex A IDL3+ of DDS-DCPS Ports and Connectors | `crates/dds4ccm/idl/ccm_dds.idl` |
| Annex B IDL for DDS-DLRL Ports and Connectors | `crates/dds4ccm/idl/ccm_dds_dlrl.idl`, `crates/dds4ccm/idl/dds_rtf2_dlrl.idl` |
| Annex C XML Schema for QoS Profiles | `crates/xml/schemas/dds4ccm_qos_profile.xsd` |
| Annex D Default QoS Profile | `crates/xml/schemas/dds4ccm_default_qos_profile.xml` |
| Annex E QoS Policies for DDS Patterns | `crates/dds4ccm/src/profiles.rs`, `crates/dds4ccm/profiles/` |

The data model of the ports and connectors (their names) lives in
`crates/ccm/src/dds4ccm.rs`, the connector fragments of §7 with their
`Container` in `crates/dds4ccm/`; the CCM container stack `crates/corba-ccm/`
(see `omg-ccm-4.0.md`) uses neither. The reader of the XML QoS profiles is in
`crates/xml/` (see `dds-xml-1.0.md` and `zerodds-xml-1.0.md`); it reads the
forms of DDS4CCM that DDS-XML 1.0 lacks with
`ReadOptions::accept_vendor_forms`. The IDL of Annex A and of Annex B is a file
in `crates/dds4ccm/idl/`; the tests read it through `crates/idl-compose/`,
`crates/idl-parse/` and `crates/idl-lower/`, the one of Annex B also through
omniidl.

---

## §1 Scope

### §1 Scope statement

**Spec:** §1, p. 1 (PDF) — "This specification defines how CCM components may
interact using DDS and how related DDS entities may be configured using CCM
configuration mechanisms."

**Repo:** interaction through DDS: `crates/dds4ccm/` (ports, connectors, DCPS
entities, threading policy). Configuration through the attributes of the
connectors and QoS profiles: `crates/dds4ccm/src/connector.rs`,
`crates/dds4ccm/src/entities.rs`, the reader in `crates/xml/`. The DLRL ports
and connectors of §8: `crates/dds4ccm/src/dlrl.rs` over the DLRL entities of
`crates/dlrl/src/cache/`.

**Tests:** `crates/dds4ccm/tests/conformance.rs::dds4ccm_2_two_containers_exchange_state_and_events_through_the_normative_ports_and_connectors`, `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`; the items of §7, §8 and of the annexes.

**Status:** partial — the DCPS part is built and tested over DDS (§2, point 1). Of the DLRL part (§2, point 2), built and green on the Linux test host: `CacheOperation` (§8.2.1.1), the DLRL connectors (§8.3), the DCPS entities of a DLRL port (§8.4.1), the assignment of the QoS profile through `topic_filter` (§8.4.2) and the IDL of Annex B. Partial: the home receptacles (§8.2.1.2) and with them the composition rule (§8.2.2), since Rust has no generated home classes (`dlrl-1.2` §8.2.2, point (3)). For point 1, hosting in the CCM container of `crates/corba-ccm` is not built (see §2).

---

## §2 Conformance

### §2 Conformance points

**Spec:** §2, p. 1 (PDF) — two conformance points:
1. "A CCM framework claiming conformance with this 'DDS for Lightweight CCM'
   specification shall support DDS-DCPS normative ports and connectors and
   their configuration."
2. "An optional compliance point [...] is the support for DLRL ports and
   connectors and their configuration."

**Repo:** point 1: `crates/dds4ccm/` — the six port types of §7.2.2.2
(`src/ports.rs`) over the basic ports of §7.2.2.1, the connectors `DDS_Base`,
`DDS_TopicBase`, `DDS_State` and `DDS_Event` (`src/connector.rs`), their DCPS
entities and QoS profiles (`src/entities.rs`), the threading policy
(`src/threading.rs`), the IDL of Annex A (`idl/ccm_dds.idl`). Point 2:
`crates/dds4ccm/src/dlrl.rs` — `CacheOperation`, `DlrlPortType` with the home
receptacles, `DlrlPort`, `DlrlConnector` over `DDS_Base`, the DCPS entities and
the QoS profile of a DLRL port, the IDL of Annex B (`idl/ccm_dds_dlrl.idl`).

**Tests:** point 1: `crates/dds4ccm/tests/conformance.rs::dds4ccm_2_two_containers_exchange_state_and_events_through_the_normative_ports_and_connectors`; each clause on its own in the items of §7.2.2 to §7.4.4 and Annex A, C, D. Point 2: `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`, clause by clause in the items of §8.2.1.1 to §8.4.2 and Annex B.

**Status:** partial — point 1 is met for the `Container` of `crates/dds4ccm`; hosting in the CCM container of `crates/corba-ccm` is not built. Point 2 (optional) is met in part. Green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131. The test of point 1 is one deployment with everything the point names: two `Container`s, the connectors `DDS_State` and `DDS_Event`, each of the six port types, the configuration through `domain_id`, `topic_name`, `key_fields` and a `qos_profile` given as a `file:` URL with `topic_filter`, a threading policy of the container. A component of one container writes a state through `DDS_Update` and events through `DDS_Write`; components of the other container observe through `DDS_Read`, `DDS_Get`, `DDS_Listen` and `DDS_StateListen` and receive through `DDS_Get` and `DDS_Listen`, over DDS. The components are Rust objects in the `Container` of `crates/dds4ccm`; the CCM container `crates/corba-ccm` does not host the fragments (it has no dependency on DDS). Point 2: built and green are `CacheOperation` (§8.2.1.1), the DLRL connectors with their mirror ports and `error_listener` (§8.3), the DCPS entities of a DLRL port (§8.4.1), the QoS profile with `topic_filter` (§8.4.2) and the IDL of Annex B with its check against `DDS::Cache` and the trait `CacheOperation`; two containers exchange objects over DDS through a DLRL port each (`dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other`). Partial: a home receptacle is the generic `ObjectHome` of its class, not a home class generated by the DLRL tooling that inherits from `DDS::ObjectHome` (§8.2.1.2, §8.2.2); there are no generated classes for Rust (`dlrl-1.2` §8.2.2, point (3)). The DLRL entities below are built in `crates/dlrl/src/cache/` (document `dlrl-1.2`).

---

## §3 Normative references

**Spec:** §3, p. 1-2 (PDF) — CORBA 3.2 (Part 1/2/3,
formal/2011-11-01..03), CCM (= CORBA Part 3), D&C (formal/06-04-02), DDS 1.2
(formal/07-07-01), XML Schema (W3C 2004).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

---

## §4 Terms and definitions

**Spec:** §4, p. 2 (PDF) — Connector, Extended Port, Fragment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — terms.

---

## §5 Symbols

**Spec:** §5, p. 2 (PDF) — CCM/CIF/CORBA/DCPS/DDS/DLRL/IDL/UML/XML.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — abbreviations.

---

## §6 Additional information

### §6.1 Changes to adopted OMG specifications

**Spec:** §6.1, p. 2 (PDF) — "None in this specification."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — "None in this specification."

### §6.2 Acknowledgements

**Spec:** §6.2, p. 3 (PDF) — editorial.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — acknowledgements.

---

## §7 DDS-DCPS extended ports and connectors

### §7.1 Introduction

#### §7.1.1 Rationale for DDS extended ports and connectors definition

**Spec:** §7.1.1, p. 5 (PDF) — rationale for DDS-specific connectors instead
of generic CCM receptacles.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rationale.

#### §7.1.2 From connector-oriented modeling to connectionless deployment

**Spec:** §7.1.2, p. 6 (PDF) — pattern: connector at design time, DCPS topic
at deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — explanation of the difference between model and execution; no requirement.

### §7.2 DDS-DCPS extended ports

#### §7.2.1 Design rules

**Spec:** §7.2.1, p. 6-7 (PDF) — four design sub-sections (§7.2.1.1
Parameterization, §7.2.1.2 Basic Ports Definition, §7.2.1.3 Interface
Design, §7.2.1.4 Simplicity vs Richness Trade-off).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — design rules of the IDL; the normative ports are in §7.2.2 and Annex A.

#### §7.2.2 Normative DDS-DCPS ports

#### §7.2.2.1 DDS-DCPS basic port interfaces

**Spec:** §7.2.2.1, p. 8-17 (PDF) — the interfaces of the basic ports:
`InstanceHandleManager`, `Writer`, `Updater` (§7.2.2.1.1); `ReadInfo`,
`QueryFilter`, `Reader`, `Getter`, `Listener`, `StateListener` and Figure 7.2
(§7.2.2.1.2); `DataListenerControl`, `StateListenerControl` (§7.2.2.1.3);
`ContentFilterSetting` (§7.2.2.1.4); `PortStatusListener`,
`ConnectorStatusListener` (§7.2.2.1.5).

**Repo:** crate `zerodds-dds4ccm`. The IDL as types and traits:
`crates/dds4ccm/src/ccm_dds.rs` (`ReadInfo`, `QueryFilter`, `AccessStatus`,
`InstanceStatus`, `ListenerMode`, `UNLIMITED`, the exceptions
`AlreadyCreated`, `NonExistent`, `InternalError`, `NonChangeable`, the
interfaces without the data type), `crates/dds4ccm/src/ccm_dds_typed.rs` (the
interfaces of `CCM_DDS::Typed<T, TSeq>`). The connector fragments over the
DCPS entities: `crates/dds4ccm/src/writer.rs::{WriterFragment, UpdaterFragment}`,
`crates/dds4ccm/src/reader.rs::{ReaderFragment, GetterFragment}`,
`crates/dds4ccm/src/reader_core.rs`, `crates/dds4ccm/src/instance_status.rs`
(Figure 7.2), `crates/dds4ccm/src/listener.rs`,
`crates/dds4ccm/src/status.rs`,
`crates/dcps/src/subscriber.rs::DataReader::read_instance_w_condition`. The
names of the interfaces in the data model:
`crates/ccm/src/dds4ccm.rs::BasicPortKind`.

**Tests:** `crates/dds4ccm/tests/publishing_ports.rs::{dds4ccm_7_2_2_1_1_instance_handle_manager_registers_and_unregisters_through_dds, dds4ccm_7_2_2_1_1_write_one_with_a_handle_that_contradicts_the_key_is_internal_error, dds4ccm_7_2_2_1_1_write_many_stops_at_the_first_error_and_reports_its_index, dds4ccm_7_2_2_1_1_coherent_write_brackets_the_batch, dds4ccm_7_2_2_1_1_create_one_of_an_existing_instance_is_already_created, dds4ccm_7_2_2_1_1_delete_one_disposes_and_update_one_needs_an_existing_instance, dds4ccm_7_2_2_1_1_update_many_reports_every_missing_index, dds4ccm_7_2_2_1_1_is_global_scope_checks_the_lifecycle_in_the_data_space, dds4ccm_7_2_2_1_1_writer_and_updater_inherit_the_instance_handle_manager}`, `crates/dds4ccm/tests/subscribing_ports.rs::{dds4ccm_7_2_2_1_2_read_last_returns_one_sample_per_instance_and_read_all_every_sample, dds4ccm_7_2_2_1_2_a_read_returns_seen_samples_again_and_alive_instances_only, dds4ccm_7_2_2_1_2_read_one_last_of_an_unknown_instance_is_non_existent, dds4ccm_7_2_2_1_2_read_one_of_a_keyless_topic_reads_the_one_instance, dds4ccm_7_2_2_1_2_a_query_filters_every_read, dds4ccm_7_2_2_1_2_get_one_returns_false_after_time_out, dds4ccm_7_2_2_1_2_get_many_respects_max_delivered_data, dds4ccm_7_2_2_1_2_listener_modes, dds4ccm_7_2_2_1_2_state_listener_calls_on_creation_on_one_update_on_deletion, dds4ccm_7_2_2_1_2_instance_status_follows_figure_7_2, dds4ccm_7_2_2_1_2_an_instance_leaving_and_reentering_the_query_is_filtered_out_and_in, dds4ccm_7_2_2_1_2_a_fresh_sample_the_query_rejected_is_not_gotten_under_a_later_query, dds4ccm_7_2_2_1_2_a_listener_gets_the_exit_from_the_query_and_the_reentry, dds4ccm_7_2_2_1_3_is_filter_interpreted_false_makes_exit_and_reentry_updates, dds4ccm_7_2_2_1_3_is_filter_interpreted_true_makes_exit_a_deletion_and_reentry_a_creation, dds4ccm_7_2_2_1_3_listener_control_defaults, dds4ccm_7_2_2_1_4_set_filter_parameters, dds4ccm_7_2_2_1_5_status_listeners, dds4ccm_7_2_2_1_5_each_dds_status_goes_to_the_operation_of_its_listener}`, `crates/dds4ccm/src/instance_status.rs::{tests::dds4ccm_7_2_2_1_2_figure_7_2_created_updated_deleted, tests::dds4ccm_7_2_2_1_2_figure_7_2_a_changed_disposed_generation_count_creates_again, tests::dds4ccm_7_2_2_1_2_a_sample_without_data_is_passed_only_as_a_deletion, tests::dds4ccm_7_2_2_1_2_data_read_after_the_disposal_keeps_the_status_of_its_time, tests::dds4ccm_7_2_2_1_2_a_sample_read_again_keeps_its_status_and_does_not_move_the_state, tests::dds4ccm_7_2_2_1_2_an_existing_instance_leaves_and_reenters_the_query, tests::dds4ccm_7_2_2_1_2_an_instance_outside_the_query_is_deleted_or_created_again, tests::dds4ccm_7_2_2_1_2_deliver_asks_the_query_for_fresh_data_and_recalls_the_reentry, tests::dds4ccm_7_2_2_1_2_access_status_follows_the_sample_state, tests::dds4ccm_7_2_2_1_2_what_is_returned_is_ordered_by_instance, tests::dds4ccm_7_2_2_1_2_instances_the_reader_purged_are_forgotten}`, `crates/dds4ccm/src/listener.rs::{tests::dds4ccm_7_2_2_1_3_is_filter_interpreted_decides_what_a_filtered_instance_triggers, tests::dds4ccm_7_2_2_1_3_an_instance_told_deleted_at_its_exit_is_not_deleted_twice, tests::dds4ccm_7_2_2_1_2_state_listener_groups_the_updates_by_the_mode}`, `crates/dds4ccm/src/writer.rs::{tests::dds4ccm_7_2_2_1_1_the_index_of_an_exception_is_the_position_in_the_sequence, tests::dds4ccm_7_2_2_1_1_the_later_of_the_observer_s_changes_is_its_latest}`, `crates/dds4ccm/src/reader_core.rs::{tests::dds4ccm_7_2_2_1_2_read_last_keeps_the_last_sample_of_each_instance, tests::dds4ccm_7_2_2_1_2_no_data_is_an_empty_sequence_and_an_error_stays_one}`, `crates/dds4ccm/src/ccm_dds.rs::{tests::dds4ccm_annex_a_unlimited_is_zero_and_the_defaults_are_those_of_the_idl, tests::dds4ccm_annex_a_internal_error_carries_the_dds_return_code_and_the_index}`, `crates/dcps/tests/read_api_spec.rs::dcps_2_2_2_5_3_14_read_instance_w_condition_reads_one_instance_through_the_condition`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_2_2_1_basic_port_interfaces_have_the_names_of_the_idl`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48, `-p zerodds-dcps --test read_api_spec` 7 of 7. The fragment computes `INSTANCE_FILTERED_OUT` and `INSTANCE_FILTERED_IN` for the query of the Reader of the port, on the path of fresh data (`Getter`, `Listener`, `StateListener`): the first fresh sample of an instance that exists for the component that the query rejects is `INSTANCE_FILTERED_OUT` and is delivered with its data; the next sample of the instance that passes the query is `INSTANCE_FILTERED_IN`. `is_filter_interpreted` TRUE makes them `on_deletion` and `on_creation`, FALSE updates; the tests check both values through DDS. The two statuses do not exist for the content filter of the port (attribute `filter`): a sample a ContentFilteredTopic rejects does not reach the DataReader. Deviation from a DDS read through a QueryCondition: the fragment reads every `NOT_READ` sample and evaluates the query itself. A fresh sample the query rejects is `READ` afterwards; `Getter` and the listeners do not deliver it under a query set later either, `Reader` returns it as `ALREADY_SEEN`. `INCONSISTENT_TOPIC`: `crates/dds4ccm/src/status.rs::TopicStatusListener` calls `on_inconsistent_topic`. No port sets this TopicListener; the topic belongs to the connector, which sets it on its topic (§7.3.1, tested through DDS there). The tests trigger `REQUESTED_DEADLINE_MISSED`, `OFFERED_DEADLINE_MISSED`, both `INCOMPATIBLE_QOS` and both `MATCHED` through DDS; for `SAMPLE_LOST`, `SAMPLE_REJECTED`, `LIVELINESS_CHANGED` and `LIVELINESS_LOST` the test calls the DDS listeners of the port the way DDS calls them, for `INCONSISTENT_TOPIC` the `TopicStatusListener`.

#### §7.2.2.2 DDS-DCPS extended ports

**Spec:** §7.2.2.2, p. 18-19 (PDF) — six port types made of the basic ports:
`DDS_Write`, `DDS_Update`, `DDS_Read`, `DDS_Get`, `DDS_Listen`,
`DDS_StateListen`; the ports of the subscribing side with the attribute
`filter` (`setraises(NonChangeable)`), `filter_config`, `dds_entity` and
`status`.

**Repo:** `crates/dds4ccm/src/ports.rs::{DdsWrite, DdsUpdate, DdsRead, DdsGet,
DdsListen, DdsStateListen}` with the basic ports under the names of the
specification (`data`, `fresh_data`, `data_control`, `data_listener`,
`filter_config`, `dds_entity`, `status`); `configuration_complete` creates the
DataReader of the port, on a ContentFilteredTopic for a filter with an
expression, and `filter` is `NonChangeable` afterwards;
`crates/dds4ccm/src/receptacle.rs`. The port types and their basic ports in
the data model: `crates/ccm/src/dds4ccm.rs::ExtendedPortKind::basic_ports`.

**Tests:** `crates/dds4ccm/tests/extended_ports.rs::{dds4ccm_7_2_2_2_each_port_type_has_the_basic_ports_of_the_specification, dds4ccm_7_2_2_2_filter_is_non_changeable_after_configuration, dds4ccm_7_2_2_2_filter_selects_what_the_reader_receives, dds4ccm_7_2_2_2_dds_entity_is_the_underlying_reader, dds4ccm_7_2_2_2_a_removed_port_deletes_its_reader}`, `crates/dds4ccm/src/receptacle.rs::tests::dds4ccm_7_2_2_2_a_receptacle_holds_one_connection`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_2_2_2_port_types_are_made_of_the_basic_ports_of_the_specification`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --test extended_ports` 5 of 5, `-p zerodds-ccm` 48 of 48.

### §7.3 DDS-DCPS connectors

#### §7.3.1 Base connectors

**Spec:** §7.3.1, p. 20 (PDF) — `DDS_Base`: `uses ConnectorStatusListener
error_listener`, attributes `domain_id` and `qos_profile` ("File URL or XML
string"). `DDS_TopicBase : DDS_Base`: attributes `topic_name` and
`key_fields`. All four attributes are `setraises(NonChangeable)`: "Any attempt
to change those attributes once the configuration is complete will raise a
NonChangeable exception."

**Repo:** `crates/dds4ccm/src/connector.rs::{DdsBase, DdsTopicBase,
ConnectorError}`; `crates/dds4ccm/src/entities.rs::{Profile, ProfileError}`
reads `qos_profile`; `crates/dds4ccm/src/status.rs::TopicStatusListener` is
the TopicListener the connector sets on its topic. Data model:
`crates/ccm/src/dds4ccm.rs::{ConnectorKind, ConnectorAttribute}`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_1_the_attributes_raise_non_changeable_once_the_configuration_is_complete, dds4ccm_7_3_1_qos_profile_is_a_file_url_or_the_xml_string, dds4ccm_7_3_1_a_profile_that_cannot_be_read_leaves_the_connector_unconfigured, dds4ccm_7_3_1_topic_name_and_key_fields, dds4ccm_7_3_1_error_listener_gets_the_inconsistent_topic_and_the_incompatible_qos, dds4ccm_7_3_1_two_connectors_of_one_topic_both_get_the_inconsistent_topic, dds4ccm_7_3_1_a_topic_that_exists_must_have_the_topic_qos_of_the_profile, dds4ccm_7_3_1_key_fields_of_a_type_with_its_type_object_are_its_key_members, dds4ccm_7_3_1_error_listener_gets_the_statuses_of_the_observer_of_a_global_update_port}`, `crates/dds4ccm/tests/subscribing_ports.rs::dds4ccm_7_2_2_1_5_each_dds_status_goes_to_the_operation_of_its_listener`, `crates/dds4ccm/src/connector.rs::{tests::dds4ccm_7_3_1_key_fields_are_compared_with_the_key_members_the_type_names, tests::dds4ccm_7_3_1_key_fields_of_a_type_that_does_not_name_its_key_are_checked_for_presence, tests::dds4ccm_7_3_1_a_topic_connector_is_completed_as_one, tests::dds4ccm_7_3_1_a_connector_error_says_what_is_wrong}`, `crates/dds4ccm/src/entities.rs::{tests::dds4ccm_7_3_1_an_xml_string_is_read_and_its_last_profile_is_the_profile, tests::dds4ccm_7_3_1_the_empty_value_is_no_profile_and_every_qos_is_the_default, tests::dds4ccm_7_3_1_a_container_without_policies_is_the_default_qos, tests::dds4ccm_7_3_1_a_file_url_is_parsed_into_path_and_profile, tests::dds4ccm_7_3_1_what_is_no_profile_document_is_an_error, tests::dds4ccm_7_3_1_the_profile_of_a_document_with_libraries, tests::dds4ccm_7_3_1_a_topic_differs_in_the_policies_the_profile_sets_to_another_value}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_1_base_connectors_have_the_attributes_of_the_idl`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48. A setter after `configuration_complete` returns `NonChangeable`; a failed `configuration_complete` leaves the attributes changeable. `INCONSISTENT_TOPIC` and both `INCOMPATIBLE_QOS` reach `error_listener` through DDS. So do the statuses of the DataReader a `DDS_Update` port with `is_global_scope` looks the instances up on (`REQUESTED_INCOMPATIBLE_QOS` tested through DDS). That port has no `PortStatusListener`: `REQUESTED_DEADLINE_MISSED` and `SAMPLE_LOST` of this DataReader go to `on_unexpected_status`; for these the test calls the DDS listener the way DDS calls it. Decisions where the specification is silent: (1) `qos_profile`: a value that starts with `<` is the XML text; a `file:` URL (`file:///path`, `file:/path`, `file://localhost/path`, with `%xx`) names the file; on Windows `file:///C:/path` names the path `C:/path` (RFC 8089 Appendix E.2); the empty value is no profile, the entities have the default QoS of DDS; any other value is an error. If the document holds several profiles, the last one directly under `<dds>` is used (§7.4.2.3.1: a profile inherits only from those read before it). If the document has profiles in `<qos_library>` elements only (the form of DDS-XML 1.0), the last profile of the last library that has one is used. If it has both, the profiles directly under `<dds>` come first, wherever the libraries stand. The fragment of the URL names another profile (`#Profile`, `#Library::Profile`). The document is read with the forms of DDS4CCM that DDS-XML 1.0 lacks (a profile directly under `<dds>`, `domainparticipant_qos`). (2) `key_fields`: the key of a topic is that of its data type. The attribute is checked against the type: with the names of the key members where the type carries its complete `TypeObject` and has no base type, otherwise by whether the type has a key. A contradiction is `ConnectorError::KeyFields`. Both ways are tested through DDS: with a type without a `TypeObject` and with a type that carries its `TypeObject`. (3) Two connectors that use a topic of one name and data type in one DomainParticipant share the topic and its TopicListener; the one removed last deletes the topic. A topic that already exists in the DomainParticipant keeps its QoS: the topic of another connector as well as a topic the application created. It serves the connector when every policy the `topic_qos` of its profile sets has the same value there; a policy that is not set is not requested. Otherwise the connector refuses the port with `ConnectorError::TopicQos`, which names the policies that differ, and creates nothing for it. Not run: a `file:` URL with a drive letter on Windows. The mapping of the path is tested on the Linux test host, reading the file on Windows is not; the Windows jobs of `ci/platform_jobs.py` do not build `zerodds-dds4ccm`.

#### §7.3.2 Pattern State Transfer

**Spec:** §7.3.2, p. 20-21 (PDF) — `connector DDS_State : DDS_TopicBase` with
the mirror ports `observable` (`DDS_Update`), `passive_observer` (`DDS_Read`),
`pull_observer` (`DDS_Get`), `push_observer` (`DDS_Listen`) and
`push_state_observer` (`DDS_StateListen`). "Typically, with this pattern,
HISTORY QoS should be set to KEEP_LAST."

**Repo:** `crates/dds4ccm/src/connector.rs::DdsState`; data model:
`crates/ccm/src/dds4ccm.rs::ConnectorKind::State`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_2_state_connector_the_observable_reaches_the_four_observers, dds4ccm_7_3_2_and_7_3_3_history_is_that_of_the_pattern_unless_the_profile_sets_it}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_2_and_7_3_3_pattern_connectors_have_the_mirror_ports_of_the_idl`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48. A mirror port is a method of the connector under the name of the specification. `observable(component, is_global_scope)` returns the port `DDS_Update`; the four observer methods are given the port of the component, whose attribute `filter` and threading policy are set, and complete its configuration. The test writes through `observable` and reads on all four observer ports through DDS. HISTORY is KEEP_LAST where the QoS profile does not set HISTORY. The connector does not apply the QoS tables of Annex E by itself; they are the profile `profiles::STATE_TRANSFER` for the attribute `qos_profile` (Annex E).

#### §7.3.3 Pattern Event Transfer

**Spec:** §7.3.3, p. 21 (PDF) — `connector DDS_Event : DDS_TopicBase` with the
mirror ports `supplier` (`DDS_Write`), `pull_consumer` (`DDS_Get`) and
`push_consumer` (`DDS_Listen`). "Typically, with this pattern, HISTORY QoS
should be set to KEEP_ALL."

**Repo:** `crates/dds4ccm/src/connector.rs::DdsEvent`; data model:
`crates/ccm/src/dds4ccm.rs::ConnectorKind::Event`.

**Tests:** `crates/dds4ccm/tests/connectors.rs::{dds4ccm_7_3_3_event_connector_the_supplier_reaches_both_consumers, dds4ccm_7_3_2_and_7_3_3_history_is_that_of_the_pattern_unless_the_profile_sets_it}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_3_2_and_7_3_3_pattern_connectors_have_the_mirror_ports_of_the_idl`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48. `supplier(component)` returns the port `DDS_Write`; `pull_consumer` and `push_consumer` are given the port of the component. The test sends through `supplier`; both consumers receive every event through DDS. HISTORY is KEEP_ALL where the QoS profile does not set HISTORY. The connector does not apply the QoS tables of Annex E by itself; they are the profile `profiles::EVENT_TRANSFER` for the attribute `qos_profile` (Annex E).

### §7.4 Configuration and QoS support

#### §7.4.1 DCPS entities

**Spec:** §7.4.1, p. 21 (PDF) — the connector fragments create the DDS
entities. A port has a DataReader and/or DataWriter, "entirely dedicated to
their port". Subscriber, Publisher and DomainParticipant "may be allocated to
the port or to the component [...] or to the container"; "components and
containers can be given DDS entities that can be used by the infrastructure
for servicing embedded ports if they meet the port requirements".

**Repo:** `crates/dds4ccm/src/entities.rs::{Container, Component}` (the
default entities per domain, given or created from the profile, and the
allocation `Component::place`); `crates/dds4ccm/src/connector.rs` (topic,
DataWriter and DataReader per port, `remove`).

**Tests:** `crates/dds4ccm/tests/entities.rs::{dds4ccm_7_4_1_each_port_has_its_reader_or_writer_and_shares_the_entities_of_the_container, dds4ccm_7_4_1_a_container_and_a_component_can_be_given_their_entities, dds4ccm_7_4_1_a_given_participant_serves_the_ports_it_suits, dds4ccm_7_4_1_a_container_whose_defaults_serve_a_port_is_not_removed, dds4ccm_7_4_1_a_port_removed_before_its_connector_leaves_the_connector_removable, dds4ccm_7_4_1_a_removal_that_failed_is_tried_again, dds4ccm_7_4_1_a_container_creates_its_participants_with_the_standard_runtime_configuration, dds4ccm_7_4_3_a_port_the_defaults_do_not_suit_gets_dedicated_entities}`, `crates/dds4ccm/tests/connectors.rs::dds4ccm_7_4_1_a_listener_called_at_the_connection_may_use_the_connector`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 117 of 117, `-p zerodds-ccm` 48 of 48. Each port has its DataReader or DataWriter. A port gets, in this order: the Publisher or Subscriber of the container with the DomainParticipant of the container; those of the component with the DomainParticipant they live in; a Publisher or Subscriber of its own in the first default DomainParticipant that suits; a DomainParticipant of its own with a Publisher or Subscriber of its own. A container always has defaults (without a profile with the default QoS of DDS); without profiles all ports of a container share its three entities. `set_default_entities` gives a container or a component a DomainParticipant, Publisher and Subscriber; they are not deleted. `remove` of the connector deletes what its fragments created, `Container::remove` the defaults that were created. A `remove` of the port or of the connector that fails (for example on a ReadCondition the application created on the DataReader) keeps what it could not delete; the next call tries it again, and a concurrent call waits for the first. The data a DataReader already holds when its push port is connected go to the listener of the component after the connector has completed the fragment and released its locks: a listener may then connect ports to the same connector or remove it (tested through DDS). `Container::remove` checks first: while a port has its DataReader or DataWriter in a default Publisher or default Subscriber that was created, or in a Publisher or Subscriber of its own in a default DomainParticipant that was created, the result is `PRECONDITION_NOT_MET`; nothing is deleted, the defaults stay, the port works on (tested through DDS). A default DomainParticipant that was created is deleted without what it contains: if a topic is still in it, it stays the default of its domain and the next call deletes it. `Container::new` creates the DomainParticipants with the standard runtime configuration, `Container::with_runtime_config` with the given one; both are tested through DDS. The tests read the allocation from the entities (Subscriber and Publisher of the reader and writer, and their DomainParticipant). What "suits" means is under §7.4.3.

#### §7.4.2 DDS QoS policies in XML

#### §7.4.2.1 XML file syntax

**Spec:** §7.4.2.1, p. 22 (PDF) — UTF-8; values without leading and trailing spaces; values are case-sensitive; comments `<!-- -->`; root `<dds>`. Table 7.1: Boolean "yes, 1, true or BOOLEAN_TRUE / no, 0, false or BOOLEAN_FALSE", "Not case-sensitive"; Enum as a string; Long and UnsignedLong decimal or hexadecimal, `LENGTH_UNLIMITED`.

**Repo:** `crates/xml/src/parser.rs` (UTF-8, comments), `crates/xml/src/zerodds_xml.rs::parse_dds_xml_with` (root `<dds>`), `crates/xml/src/qos_parser.rs` (`bool_value`, `table_7_1_hex`, `limit_value`, `positive_long`, `idl_long_value`, `partition_name`, the enumeration literals per policy), `crates/xml/src/types.rs` (`parse_bool_dds4ccm`, `parse_long`, `parse_ulong`), `crates/xml/src/options.rs` (`VendorForms`, `Ctx::table_7_1_form_outside_schema`)

**Tests:** `crates/xml/src/parser.rs::tests::parse_minimal_document`, `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`, `crates/xml/src/parser.rs::tests::non_utf8_declared_encoding_rejected`, `crates/xml/src/parser.rs::tests::comments_stripped`, `crates/xml/src/parser.rs::tests::attributes_preserved`, `crates/xml/src/parser.rs::tests::invalid_xml_rejected`, `crates/xml/src/qos_parser.rs::tests::rejects_non_dds_root`, `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_1_leading_and_trailing_spaces_are_removed, dds4ccm_7_4_2_1_enums_are_case_sensitive_strings, dds4ccm_7_4_2_1_table_7_1_long_is_decimal_hexadecimal_or_length_unlimited, dds4ccm_7_4_2_1_the_root_is_dds, dds4ccm_7_4_2_1_table_7_1_the_default_reading_names_the_form}`, `crates/xml/tests/spec_items.rs::numbers_outside_the_schemas_types_are_refused`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548. UTF-8 (another declared encoding is refused), comments at any place, the root `<dds>` (a profile or a single QoS as the root is refused; the root of Annex D with namespace and `xsi:schemaLocation` is read). Leading and trailing spaces are removed before every value is read; the test reads a profile with spaces, tab and line feed around every value and compares it with the profile without them. For partition names DDS-XML 1.0 §7.1.4 keeps the spaces as part of the string; the reading of DDS4CCM (`ReadOptions::accept_vendor_forms`, which is how the connectors of `crates/dds4ccm` read) removes them and reports it. Enumeration literals are case-sensitive, a number is no enum (`XmlError::BadEnum`, under both readings). Long and UnsignedLong of Table 7.1: decimal, hexadecimal, `LENGTH_UNLIMITED` in every field of that type. The hexadecimal number and the negative Long are in Table 7.1 of DDS-XML 1.0 §7.1.4 as well. The schema of DDS-XML 1.0 leaves them out of every numeric field of a QoS policy: `positiveInteger_UNLIMITED`, `nonNegativeInteger_Duration_*`, `xs:positiveInteger` and `xs:nonNegativeInteger` have decimal digits only, `xs:nonNegativeInteger` no negative value. Text and schema of DDS-XML 1.0 disagree here; the default reading follows the schema. One rule holds for all numeric fields: the hexadecimal number in `history/depth`, `durability_service/history_depth`, the limits of `resource_limits` and `durability_service`, `resource_limits/initial_samples` and `initial_instances`, `sec`/`nanosec` of a duration, `ownership_strength/value` and `transport_priority/value`, and a negative value and `LENGTH_UNLIMITED` in the last two (`long` in the IDL), is a vendor form. The reading of DDS4CCM reads and reports it (one report per element), the default reading refuses it with `XmlError::VendorForm`; the message names Table 7.1 of both specifications and the schema type. The Booleans of DDS4CCM are no form of DDS-XML 1.0, and their message says so. Values outside the range the IDL gives the field stay an error (a resource limit or `history/depth` of 0 or `0x80000000`, a duration of `0x80000000` seconds, more than 32 bits). Booleans: item §7.4.2.1 Table 7.1. "Legal values are alphanumeric characters" is not checked on its own: the examples of the specification itself write `_`, `*` and `-` in values; every value is read by its type.

#### §7.4.2.1 Table 7.1 — Boolean `yes`/`no`/`BOOLEAN_TRUE`/`BOOLEAN_FALSE` regardless of case

**Spec:** §7.4.2.1, Table 7.1, p. 22 (PDF) — "Boolean | yes, 1, true or BOOLEAN_TRUE: / no, 0, false or BOOLEAN_FALSE: | Not case-sensitive".

**Repo:** `crates/xml/src/types.rs::parse_bool_dds4ccm`, `crates/xml/src/qos_parser.rs::bool_value` (called for `coherent_access`, `ordered_access`, `autoenable_created_entities`, `autodispose_unregistered_instances`)

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_1_table_7_1_boolean_forms_are_read_without_regard_to_case, dds4ccm_7_4_2_1_table_7_1_other_boolean_text_is_refused, dds4ccm_7_4_2_1_table_7_1_the_default_reading_names_the_form}`, `crates/xml/src/types.rs::tests::dds4ccm_7_4_2_1_table_7_1_bool_forms_in_any_case`, `crates/xml/src/qos_parser.rs::tests::boolean_case_sensitive_rejected`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548. The four Boolean fields of a QoS profile read `yes`, `1`, `true`, `BOOLEAN_TRUE` and `no`, `0`, `false`, `BOOLEAN_FALSE` in any case; the test checks every form in all four fields with the value in `EntityQos`. `true`, `1`, `false`, `0` are also forms of DDS-XML 1.0 §7.1.4 and are read without a report; the others are no form there: the reading of DDS4CCM (`ReadOptions::accept_vendor_forms`) reads them and reports each with the path of its element, the default reading refuses them with `XmlError::VendorForm` and names element and text. Any other text (`on`, `y`, `2`, empty) ends with `XmlError::ValueOutOfRange` under both readings. Annex C types the fields `xs:boolean`; `xmllint` refuses `yes` and `TRUE`, the reader follows the text of Table 7.1 (rows `boolean_yes` and `boolean_upper_case` in `dds4ccm_annex_c_what_the_schema_rejects_we_reject`).

#### §7.4.2.2 Entity QoS

**Spec:** §7.4.2.2, p. 22-25 (PDF) — the six QoS containers
(`participant_qos`, `topic_qos`, `publisher_qos`, `subscriber_qos`,
`datawriter_qos`, `datareader_qos`), their policies "1-to-1" from the IDL of
DDS (§7.4.2.2.1), sequences of octets as a list of decimal or hexadecimal
values (§7.4.2.2.2), sequences of strings as an `<element>` list (§7.4.2.2.3),
enums as strings (§7.4.2.2.4), durations with `sec` and `nanosec` and their
constants (§7.4.2.2.5); a QoS with a `name` inherits through `base_name`.

**Repo:** `crates/xml/src/qos_parser.rs` (the six containers, the policies,
`vendor_container_name` for `<domainparticipant_qos>`),
`crates/xml/src/qos.rs::{QosProfile, EntityQos}`, `crates/xml/src/types.rs`
(durations, numbers, octets).

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_c_what_the_schema_accepts_we_read, dds4ccm_annex_c_what_the_schema_rejects_we_reject, dds4ccm_7_4_2_the_forms_of_the_text_the_schema_rejects_have_their_values, dds4ccm_7_4_2_2_2_octets_are_the_list_of_the_text_or_the_base64_of_the_schema, dds4ccm_7_4_2_3_profiles_inherit_select_by_topic_filter_and_stand_for_a_single_qos}`, `crates/xml/src/qos.rs::tests::{entity_qos_default_is_all_none, entity_qos_into_writer_uses_defaults_for_unset, entity_qos_into_reader_uses_defaults_for_unset, merge_override_replaces_field, merge_none_does_not_clobber}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548. The test reads every policy the schema of Annex C declares, in each of the six containers, and compares each field with the value of the document; `xmllint` confirms that the schema accepts the document. Contradictions inside the specification and how the reader resolves them: (1) §7.4.2.2 names `<participant_qos>`, Annex C and Annex D `<domainparticipant_qos>`: both are read into the same container. (2) §7.4.2.2.2 writes octets as a list, Annex C types them `xs:base64Binary`: a content in the shape of the list is the list, any other is Base64; a content that can be both is the list of the text. (3) §7.4.2.2.5 names `DURATION_ZERO_SEC` and `DURATION_ZERO_NSEC`, the patterns of Annex C do not: read. (4) §7.4.2.2 identifies names "by its fully qualified name in C++ style", the pattern `elementName` of Annex C has neither `_` nor `:`: names are read as written. (5) `transport_priority/value` is a `long` in the IDL and Table 7.1 gives a Long its negative half, Annex C types it `xs:nonNegativeInteger`: a negative value is read (reported as a vendor form; the default reading refuses it as the schema of DDS-XML 1.0 does), the same for `ownership_strength/value`. `<domainparticipant_qos>` is no form of DDS-XML 1.0; the reader takes it with `ReadOptions::accept_vendor_forms`, which is how the connectors of `crates/dds4ccm` read. The schema comparisons need `xmllint` (libxml2-utils); without it these tests return unchecked (`[skip]`). `ci/Dockerfile.rust` installs it in the image `ci-rust` of the merge lane; until that image is rebuilt, the comparisons return unchecked in CI.

#### §7.4.2.3 QoS profiles

**Spec:** §7.4.2.3, p. 25-27 (PDF) — `<qos_profile name="...">` with the six
containers; `base_name`: a profile inherits from a profile read before it
(§7.4.2.3.1); `topic_filter` selects among several QoS of one kind, a QoS
without the attribute has the filter `*` (§7.4.2.3.2); a single QoS with a
`name` stands for a profile of that name (§7.4.2.3.3).

**Repo:** `crates/xml/src/qos_parser.rs::parse_ccm_top_level_profile` (profiles
and single QoS directly under `<dds>`), `crates/xml/src/qos.rs` (`base_name`,
`topic_filter`, selection by topic).

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_7_4_2_3_profiles_inherit_select_by_topic_filter_and_stand_for_a_single_qos, dds4ccm_7_4_2_3_the_forms_of_dds4ccm_are_read_on_request_and_reported}`, `crates/xml/src/zerodds_xml.rs::tests::ccm_profiles_directly_under_dds_are_a_vendor_form`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`, `crates/xml/src/qos.rs::tests::{library_profile_lookup, glob_star_matches_all, glob_prefix, glob_question_mark, glob_exact}`, `crates/dds4ccm/src/entities.rs::tests::dds4ccm_7_4_3_the_topic_filter_allocates_the_values_to_the_entities_of_a_topic`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548, `-p zerodds-dds4ccm --all-targets` 113 of 113. The tests read the examples of the text in the form of DDS4CCM: a profile inherits only from one read before it; `topic_filter` selects, a QoS without the attribute comes after the others; a single QoS stands for a profile of its name. Profiles directly under `<dds>` are no form of DDS-XML 1.0: without `ReadOptions::accept_vendor_forms` the reader refuses them and names the form, with the option it reports each one. Contradictions inside the specification: (1) §7.4.2.3.2 writes `topic_filter="A*"`, the pattern `topicNameFilter` of Annex C has no `*`: read. (2) The examples of the text have no namespace, Annex C requires `http://www.omg.org/dds/`: elements are read by their local name. (3) The example of §7.4.2.3.3 writes `<writer_qos>` and calls it equivalent to `<datawriter_qos>`; no other place of the specification and not Annex C has the tag: refused.

#### §7.4.3 Use of QoS profiles

**Spec:** §7.4.3, p. 27 (PDF) — the profile of the connector "should contain
all values for initializing DDS Entities that are required by the connector";
`topic_filter` allocates the values with several topics. A profile of a
component defines the "component's default DomainParticipant, Subscriber,
and/or Publisher", which are preferred "if their setting is compatible with
the QoS requested in the connector's profile"; otherwise "specific entities
dedicated to the 'non-compatible' port will be created"; `topic_qos`,
`datareader_qos` and `datawriter_qos` of that profile are ignored. A profile
of a container defines the "container's defaults that should be used in
priority, if suitable".

**Repo:** `crates/dds4ccm/src/entities.rs::{Profile, Container::set_qos_profile,
Component::set_qos_profile, Component::place}`;
`crates/dds4ccm/src/connector.rs` (the QoS of topic, DataWriter and
DataReader from the profile of the connector).

**Tests:** `crates/dds4ccm/tests/entities.rs::{dds4ccm_7_4_3_the_connector_profile_initialises_every_entity, dds4ccm_7_4_3_a_port_the_defaults_do_not_suit_gets_dedicated_entities, dds4ccm_7_4_3_the_defaults_of_the_container_come_first_then_those_of_the_component, dds4ccm_7_4_3_a_component_profile_defines_the_default_participant_of_the_component, dds4ccm_7_4_3_what_cannot_be_configured_is_refused, dds4ccm_7_4_3_a_refused_port_keeps_no_entity_of_the_connector, dds4ccm_7_4_3_a_global_update_port_has_both_entities_in_its_dedicated_participant}`, `crates/dds4ccm/tests/connectors.rs::dds4ccm_7_3_1_a_topic_that_exists_must_have_the_topic_qos_of_the_profile`, `crates/dds4ccm/src/entities.rs::{tests::dds4ccm_7_4_3_the_topic_filter_allocates_the_values_to_the_entities_of_a_topic, tests::dds4ccm_7_4_3_a_default_entity_is_compatible_when_it_has_what_the_profile_sets}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48. Topic, DataWriter, DataReader, Publisher, Subscriber and DomainParticipant of a port have every policy the connector's profile sets with its value (the test reads them from the entities). The connector creates the DataWriter and the DataReader with the profile. It creates the Publisher, the Subscriber, the DomainParticipant and the topic with the profile, or takes an existing entity that suits. An existing topic that does not suit refuses the port (§7.3.1, decision 3): a topic exists once per name and data type in a DomainParticipant. `topic_filter` selects the containers by the topic name of the connector. Decisions where the specification is silent: (1) "compatible": a default entity suits when every policy the connector's profile sets for that kind of entity has the same value in the QoS of the entity; a policy that is not set is not requested; a DomainParticipant must be in the domain of the connector. (2) Order: first the defaults of the container ("in priority"), then those of the component, then an entity of its own for the port. (3) A component has a default DomainParticipant when its profile has a `domainparticipant_qos`, and a Publisher and a Subscriber in it; if its profile has only a `publisher_qos` or a `subscriber_qos`, that default entity lives in the DomainParticipant of the container. (4) The profile of a container or a component is fixed once a port asked for its defaults (`LevelError::NonChangeable`). (5) A `DDS_Update` port with `is_global_scope` has two entities: its DataWriter and the DataReader it looks the instances up on. Each is allocated as above; a DomainParticipant of the port's own holds both, the DataReader in a Subscriber of its own.

#### §7.4.4 Other configuration — threading policy

**Spec:** §7.4.4, p. 27 (PDF) — the threading policy is local to the component
and is set at the port ("for all its facets"), the component or the
container. A facet uses the thread pool of the port; "if there is no port's
policy, the component's threadpool is used"; "if there is no component's one,
the container's threadpool is used"; "if there is no container's policy, then
the default is applied".

**Repo:** `crates/dds4ccm/src/threading.rs::{ThreadingPolicy, ThreadPool}`;
`set_threading_policy` on the ports (`crates/dds4ccm/src/ports.rs`), on
`Component` and `Container` (`crates/dds4ccm/src/entities.rs`); the facets
run through the pool in `crates/dds4ccm/src/listener.rs` (`data_listener`) and
`crates/dds4ccm/src/status.rs` (`status`). Data model:
`crates/ccm/src/dds4ccm.rs::ThreadingLevel`.

**Tests:** `crates/dds4ccm/tests/threading.rs::{dds4ccm_7_4_4_a_port_policy_wins, dds4ccm_7_4_4_without_a_port_policy_the_pool_of_the_component_is_used, dds4ccm_7_4_4_without_a_component_policy_the_pool_of_the_container_is_used, dds4ccm_7_4_4_without_any_policy_the_default_applies, dds4ccm_7_4_4_the_status_facet_is_called_on_the_pool_of_the_port, dds4ccm_7_4_4_a_policy_holds_for_the_facets_activated_after_it}`, `crates/dds4ccm/src/threading.rs::{tests::dds4ccm_7_4_4_a_pool_has_the_threads_of_its_policy_under_its_name, tests::dds4ccm_7_4_4_without_a_pool_a_facet_is_called_on_the_thread_of_the_event, tests::dds4ccm_7_4_4_the_facets_of_one_port_are_called_in_order_and_one_at_a_time, tests::dds4ccm_7_4_4_two_ports_of_one_pool_are_served_at_once, tests::dds4ccm_7_4_4_a_panicking_facet_does_not_cost_the_pool_its_thread, tests::dds4ccm_7_4_4_a_pool_let_go_of_runs_what_is_queued_and_ends_its_threads}`, `crates/ccm/src/dds4ccm.rs::tests::dds4ccm_7_4_4_threading_levels_fall_back_from_port_to_component_to_container`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-ccm` 48 of 48. The tests record the thread `on_one_data` and `on_requested_deadline_missed` are called on. Decisions where the specification is silent: (1) A policy is the number of threads of a pool. (2) The facets of a port are `data_listener` and `status`. `error_listener` is a receptacle of the connector and is called on the thread DDS reports the status on. (3) The default without any policy: no pool; the facet is called on the thread that brings the event (the thread DDS reports on, or the thread of the caller for data the port already holds when the listener is enabled). (4) The calls of one port are made one after the other and in their order, also on a pool of several threads. (5) The policy of a port is `NonChangeable` after `configuration_complete`; that of a component or a container holds for the ports activated after it.

---

## §8 DDS-DLRL extended ports and connectors

### §8.1 Design principles

#### §8.1.1 Scope of DLRL extended ports

**Spec:** §8.1.1, p. 29 (PDF) — DLRL-port scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — design principle; the normative port is in §8.2 and Annex B.

#### §8.1.2 Scope of DLRL connectors

**Spec:** §8.1.2, p. 29 (PDF) — DLRL-connector scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — design principle; the connectors are in §8.3 and Annex B.

### §8.2 DDS-DLRL extended ports

#### §8.2.1 DLRL basic ports

#### §8.2.1.1 Cache operation

**Spec:** §8.2.1.1, p. 30 (PDF) — `local interface CacheOperation`: the operative subset of `DDS::Cache` (attributes `cache_usage`, `objects`, `updates_enabled`, `homes`, `sub_accesses`, `listeners`; operations `refresh`, `attach_listener`, `detach_listener`, `enable_updates`, `disable_updates`, `create_access`, `delete_access`), "mimic exactly the DDS::Cache ones".

**Repo:** `crates/dds4ccm/src/dlrl.rs::CacheOperation`: the six attributes as getters, the seven operations, each with the signature of the method of `zerodds_dlrl::cache::Cache` of the same name; `Cache` implements the trait by these methods. The exceptions are the variants of `DlrlException`: `refresh` raises `DCPSError`, `create_access` and `delete_access` `PreconditionNotMet`; `AlreadyDeleted` is in no `raises` clause and comes from every operation once the Cache is deleted (DDS 1.2 §8.2.1.1), as after `remove` of the connector. The receptacle: `crates/dds4ccm/src/dlrl.rs::DlrlPort::cache`. The former data model (`DlrlPort`, `DlrlPortKind` in `crates/ccm/src/dds4ccm.rs`) is removed.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_2_1_1_cache_operation_is_the_operative_part_of_the_cache_of_the_port`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131. Checked over two containers: the attributes are those of the Cache of the port (`DlrlConnector::cache_of`); in manual mode `refresh` applies the updates that arrived, without listener; `attach_listener` and `detach_listener` act on the Cache; `disable_updates` and `enable_updates` call `on_updates_disabled` and `on_updates_enabled`, and an update then comes with a round (`on_begin_updates`, `on_end_updates`); `create_access` and `delete_access` act on `sub_accesses`, `PreconditionNotMet` for a purpose that writes on a READ_ONLY Cache and for the CacheAccess of another Cache; after `remove` of the connector every operation raises `AlreadyDeleted`, the attributes keep their value. The IDL of Annex B and its check against the trait: see Annex B.

#### §8.2.1.2 DLRL class (ObjectHome)

**Spec:** §8.2.1.2, p. 30 (PDF) — per DLRL class a receptacle of the type of its home, which inherits from `DDS::ObjectHome` and is generated by the DLRL tooling; "All accesses to the DLRL objects of this type will be manageable through this entry point."

**Repo:** `crates/dds4ccm/src/dlrl.rs::{HomeReceptacle, DlrlPortType, DlrlPort::home}`: a home receptacle per class of the object model, declared with the name of the class; `DlrlPort::home` gives, for the name of the receptacle, the `zerodds_dlrl::cache::ObjectHome` of the class. The DLRL generator of `crates/dlrl` (`generation::generate`, DDS 1.2 §8.2.2.1) makes the object model with its mapping from the model description and the model tags; `DlrlPortType::new` takes only classes of that model. Per home receptacle the fragment creates the home of its class and registers it with the Cache of the port, in the order of the receptacles. Rust has no type per home that inherits from `ObjectHome` and is generated by the DLRL tooling (`FooHome`): the receptacle is the generic type `ObjectHome`, selected by the name of the class. The implied IDL (`zerodds-idlc dlrl --lang idl`) declares `FooHome` over `DDS::ObjectHome`; `crates/dlrl-codegen` writes home classes for C++, Java, C# and TypeScript.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_2_1_2_each_home_receptacle_is_the_home_of_its_class_of_the_generated_model, dds4ccm_8_2_2_a_port_type_has_one_cache_operation_receptacle_and_a_home_receptacle_per_class}`.

**Status:** partial — built and green on the Linux test host (`cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131): per class a home receptacle that is the home of that class registered with the Cache of the port. Open: the receptacle is not of the type of a generated home class that inherits from `DDS::ObjectHome` but of the generic type `ObjectHome`. Missing are the classes generated for Rust by DDS 1.2 §8.1.6.6 (`FooHome` with `create_object(access)` and the other typed operations, with `Foo`, `FooListener`, `FooSelection`, `FooSelectionListener`, `FooFilter`, `FooQuery`); they are open in `dlrl-1.2` §8.2.2, point (3). With them the receptacle becomes of type `FooHome`. The DLRL generator makes the model of the test from a model description and model tags. Checked: each home receptacle is the home of its class registered with the Cache of the port (`find_home_by_name`, `registration_index`), with the mapping of the port type; objects of the class are created and found through it (`get_objects`, `find_object`), it names their topics, and an object that arrives in the Cache is reported by its `ObjectListener`; a name the port type does not declare gives no home; a class the model does not have is refused by `DlrlPortType::new` (`UnknownClass`).

#### §8.2.2 DLRL extended ports composition rule

**Spec:** §8.2.2, p. 31 (PDF) — a DLRL port is made of a `CacheOperation` receptacle and one receptacle per `ObjectHome`; the fragment creates the cache by `cache_usage`, registers the homes, applies the QoS profile and enables the infrastructure.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlPortType::new` (one `CacheOperation` receptacle, at least one home receptacle, one per class, of the generic type `ObjectHome`, see §8.2.1.2; the names are IDL identifiers, collide with no IDL keyword, also not without case (IDL 4.2 §7.2.4, the table `KEYWORDS` of `crates/idl-cst`), and differ also without case), `crates/dds4ccm/src/dlrl.rs::DlrlConnector::{connect, complete}`: (1) the Cache for `cache_usage` (a readonly attribute of the interface; the fragment is given it as the mirror port `observable` is given the readonly attribute `is_global_scope`); (2) per home receptacle the home, `register_home`, `register_all_for_pubsub`; (3) the QoS profile: the `topic_qos` of each topic before `register_all_for_pubsub` as the QoS of a Topic the Cache creates (`crates/dlrl/src/cache/mod.rs::Cache::set_new_topic_qos`; a Topic is enabled when it is created); a topic the DomainParticipant has when the Cache creates its topics (of another port, also one connected at the same time, or of the application) keeps its QoS and serves the port when every policy the `topic_qos` of the profile sets has the same value there (otherwise `ConnectorError::TopicQos`, as for the DCPS connectors); then `publisher_qos`, `subscriber_qos`, `datawriter_qos` and `datareader_qos` over the QoS the Cache gave the entities that are not yet enabled (`crates/dds4ccm/src/entities.rs::Profile::{publisher_qos_over, subscriber_qos_over, datawriter_qos_over, datareader_qos_over}`); a policy the profile does not set keeps the value of the Cache; `topic_filter` selects the containers of a topic by its name in the mapping, the DomainParticipant, the Publisher and the Subscriber take the container of their kind that is selected for every topic of the port (see §8.4.2); (4) `enable_all_for_pubsub`. A refused fragment deletes what it created.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other, dds4ccm_8_2_2_a_port_type_has_one_cache_operation_receptacle_and_a_home_receptacle_per_class, dds4ccm_8_2_2_a_refused_fragment_leaves_no_entity, dds4ccm_8_2_2_ports_connected_at_the_same_time_are_judged_by_the_topics_they_meet, dds4ccm_8_4_2_topic_filter_assigns_the_values_of_the_profile_to_the_entities_of_each_topic}`, `crates/dlrl/tests/cache.rs::{dlrl_8_1_5_3_the_qos_of_a_topic_is_given_before_register_all_for_pubsub, dlrl_8_1_5_3_a_qos_for_a_new_topic_leaves_a_topic_that_exists_as_it_is}`.

**Status:** partial — built and green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 132 passed, 2 skipped (the two omniidl tests with `#[ignore]`, Annex B); `-p zerodds-dlrl --all-targets` 140 passed, 1 skipped. Open: the second point of the rule, "As many DDS:ObjectHome-derived receptacles as DLRL object types", is built with receptacles of the generic type `ObjectHome`; the home classes the DLRL tooling generates for Rust are missing (see §8.2.1.2, `dlrl-1.2` §8.2.2, point (3)). Checked: two containers with a connector each; the port of the one (WRITE_ONLY) writes a radar and a track with a relation to the radar before the port of the other (READ_ONLY) exists; both arrive there as objects with their values, their relation and their home, over DDS. The Cache has the usage of the port, the homes in the order of the receptacles, the DCPS state ENABLED; the values of the profile (TRANSIENT_LOCAL for topic, DataWriter and DataReader, KEEP_ALL for the DataWriters, a partition for Publisher and Subscriber) are on the entities, and the DataReaders stay RELIABLE where the profile is silent. The declaration refuses: no home receptacle, two receptacles of one class, a class of another model, duplicate names (also when they differ only in case), names that are no identifiers, and names that collide with an IDL keyword (`home`, `Uses`, `PortType`, `LOCAL`); a name that only contains a keyword (`HomePort`) is accepted. The assignment of the profile through `topic_filter`: see §8.4.2. A class whose relation target has no home receptacle is refused by the Cache (`BadHomeDefinition`); a TRANSIENT_LOCAL topic that exists serves a profile without `topic_qos` and refuses one with VOLATILE (`TopicQos`); a refused fragment leaves no entity in the DomainParticipant. A port with TRANSIENT_LOCAL topics in its profile and one without `topic_qos` are connected at the same time in one DomainParticipant, in 40 rounds: the second is accepted in every round; the first is accepted, with TRANSIENT_LOCAL topics, or refused with `TopicQos` for a topic the second created VOLATILE, never with an error of the DLRL entities.

### §8.3 DDS-DLRL connectors

**Spec:** §8.3, p. 31 (PDF) — a DLRL connector inherits from `DDS_Base` (`error_listener`, `domain_id`, `qos_profile`) and has one mirror port per DLRL port.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector`: dereferences to `DdsBase` (`error_listener`, `domain_id`, `qos_profile`, `configuration_complete`, `NonChangeable` afterwards); `new` with the mirror ports (at least one, distinct identifiers), `mirror_ports`, `connect` with the name of the mirror port (`ConnectorError::UnknownMirrorPort`, `NotConfigured` before `configuration_complete`), `cache_of` (the Cache of a port, for the configuration), `remove` (deletes the Caches with their entities and the DomainParticipants dedicated to a port; afterwards the `CacheOperation` of a port raises `AlreadyDeleted`). `error_listener` gets the statuses of the DCPS entities of the ports: `crates/dds4ccm/src/status.rs::WriterStatusListener` on each DataWriter, `SubscriberStatusListener` on the Subscriber of the Cache (the statuses of the DataReaders but DATA_AVAILABLE, which the Cache takes, go up to the Subscriber; REQUESTED_DEADLINE_MISSED and SAMPLE_LOST with `on_unexpected_status`, a DLRL port has no `PortStatusListener`), `crates/dds4ccm/src/connector.rs::share_topic_listener` (the `TopicListener` of a topic, shared with the other connectors of the topic in the DomainParticipant; a connector with two ports there is called once).

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_3_a_dlrl_connector_is_a_dds_base_with_a_mirror_port_per_dlrl_port, dds4ccm_8_3_error_listener_gets_the_statuses_of_the_dcps_entities_of_the_ports, dds4ccm_8_3_two_ports_of_a_connector_in_one_participant_hear_inconsistent_topic_once}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131. Checked: the declaration refuses no mirror port, duplicate and invalid names, also one that collides with an IDL keyword (`MirrorPort`); the attributes of `DDS_Base` can be set until `configuration_complete`, `NonChangeable` afterwards; `connect` before gives `NotConfigured`, an unknown mirror port `UnknownMirrorPort`; two mirror ports of two port types that share the topic `Radar` exchange an object; the container keeps its defaults while a port uses them (`PRECONDITION_NOT_MET`), after `remove` of the connector it can be removed. `error_listener` gets PUBLICATION_MATCHED and SUBSCRIPTION_MATCHED (`on_unexpected_status`), OFFERED_INCOMPATIBLE_QOS from the DataWriter of a port and REQUESTED_INCOMPATIBLE_QOS from its DataReader against entities of another participant, INCONSISTENT_TOPIC; the Cache still applies the updates. Two ports of one connector in one DomainParticipant and a port of a second connector there: an INCONSISTENT_TOPIC reaches each `error_listener` exactly once (without the check in `TopicStatusListener` it reaches the first one twice, as observed on the Linux test host).

### §8.4 Configuration and QoS support

#### §8.4.1 DDS entities

**Spec:** §8.4.1, p. 31-32 (PDF) — a DLRL port has its own Publisher and/or Subscriber (by `cache_usage`) and per topic of the DLRL objects a DataReader and/or DataWriter.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector::connect` through `zerodds_dlrl::cache::{CacheFactory::create_cache, Cache::register_all_for_pubsub}` (a Publisher with `WRITE_ONLY` and `READ_WRITE`, a Subscriber with `READ_ONLY` and `READ_WRITE`, per topic of the classes of the port a DataWriter and a DataReader in them); the DomainParticipant by §7.4.1 and §7.4.3: `crates/dds4ccm/src/entities.rs::Component::place_participant` (the container's default, else the component's, if it has what the `domainparticipant_qos` of the profile sets, else one dedicated to the port). The default Publishers and Subscribers of container and component serve no DLRL port.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::dds4ccm_8_4_1_a_dlrl_port_has_its_own_publisher_and_subscriber_and_an_entity_per_topic`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131. Checked on the DomainParticipant of the container with a given default Publisher and Subscriber: a READ_WRITE port adds a Publisher, a Subscriber and per topic a DataWriter and a DataReader, a READ_ONLY port only a Subscriber and the DataReaders, a WRITE_ONLY port only a Publisher and the DataWriters; each port has its own, the defaults stay without endpoint; the topics exist once; a profile with a `domainparticipant_qos` the default does not have gets a dedicated DomainParticipant with that QoS; after `remove` the Publishers, Subscribers and topics of the ports are gone.

#### §8.4.2 Use of QoS profiles

**Spec:** §8.4.2, p. 32 (PDF) — as §7.4.2 and §7.4.3; the profile of the DLRL connector holds the values of all topics, `topic_filter` allocates them.

**Repo:** `crates/dds4ccm/src/dlrl.rs::DlrlConnector::{connect, complete}` with the QoS profile reader of the DCPS connectors (`crates/dds4ccm/src/entities.rs::Profile`, §7.4.2, §7.4.3): the profile of the connector holds for all its ports and topics; `topic_filter` selects for each topic of the port, by its name in the mapping, the `topic_qos` (before `register_all_for_pubsub`), the `datawriter_qos` and the `datareader_qos` (before `enable_all_for_pubsub`). The DomainParticipant, the Publisher and the Subscriber serve all topics of the port: each takes the container of its kind that is selected for every topic (a container without `topic_filter` or one whose filter matches all topics), none when none is selected for any topic (`Profile::{participant_of_topics, publisher_qos_over, subscriber_qos_over}`). When the profile selects different containers of one of these kinds for two topics of the port, the connector refuses the port with `ConnectorError::TopicFilter`; the profile does not say which of them the one entity takes. A container whose filter matches no topic of the port does not apply to it.

**Tests:** `crates/dds4ccm/tests/dlrl_ports.rs::{dds4ccm_8_4_2_topic_filter_assigns_the_values_of_the_profile_to_the_entities_of_each_topic, dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131. Checked with one profile for the topics `Radar` and `Track`: TRANSIENT_LOCAL only on the topic `Track`; KEEP_ALL on the DataWriter of `Track`, KEEP_LAST 3 from the container without filter on that of `Radar`; KEEP_LAST 5 on the DataReader of `Radar` (`topic_filter="Rad*"`), the DataReader of `Track` keeps KEEP_LAST 1, both RELIABLE from the Cache; the partition of the `publisher_qos` without filter on the Publisher, that of the `subscriber_qos` with `topic_filter="*"` on the Subscriber. A `publisher_qos` for `Track` only does not apply to the Publisher of a port with `Radar` only; the connector refuses a WRITE_ONLY port with `Radar` and `Track` with `TopicFilter`, leaving no entity, and accepts a READ_ONLY port without Publisher. A `domainparticipant_qos` for `Track` only: a port with `Radar` only gets the default DomainParticipant of the container, a port with both topics `TopicFilter`.

---

## Annex A — IDL3+ of DDS-DCPS ports and connectors

**Spec:** Annex A, p. 33-40 (PDF, normative) — the IDL3+ of module `CCM_DDS`:
the part without a data type (types, constant `UNLIMITED`, exceptions,
interfaces, the connector bases `DDS_Base` and `DDS_TopicBase`) and the
template module `Typed<typename T, sequence<T> TSeq>` with the interfaces,
port types and connectors for one data type; `#include "dds_rtf2_dcps.idl"`.

**Repo:** `crates/dds4ccm/idl/ccm_dds.idl` (the text of the annex; one token
corrected: `DDS::Duration_ttime_out` is `DDS::Duration_t time_out`),
`crates/dds4ccm/idl/dds_rtf2_dcps.idl` (the types of module `DDS` the annex
refers to); the Rust mapping `crates/dds4ccm/src/ccm_dds.rs`,
`crates/dds4ccm/src/ccm_dds_typed.rs`; in the IDL frontend
`crates/idl-lower/src/lower.rs`, `crates/idl-lower/src/scope.rs` (the body of a
template module sees the scope of its declaration; a sequence formal over an
earlier formal, idl-4.2 §7.4.12.4).

**Tests:** `crates/dds4ccm/tests/annex_idl.rs::{dds4ccm_annex_a_the_non_typed_part_of_ccm_dds_compiles_to_the_declarations_of_the_annex, dds4ccm_annex_a_instantiating_typed_gives_the_interfaces_for_a_data_type, dds4ccm_annex_a_the_port_types_and_connectors_of_an_instance_are_those_of_the_annex, dds4ccm_annex_a_a_port_gives_a_component_its_basic_ports_and_a_connector_their_mirror, dds4ccm_annex_a_the_rust_interfaces_have_the_operations_of_the_idl}`, `crates/dds4ccm/src/ccm_dds.rs::{tests::dds4ccm_annex_a_unlimited_is_zero_and_the_defaults_are_those_of_the_idl, tests::dds4ccm_annex_a_internal_error_carries_the_dds_return_code_and_the_index}`, `crates/idl-lower/tests/idl_4_2_rules.rs::{idl_7_4_12_4_2_a_template_body_names_what_the_scope_of_its_declaration_declares, idl_7_4_12_4_1_a_sequence_formal_over_an_earlier_formal_is_the_sequence_passed_for_it, idl_7_4_12_4_2_an_actual_that_is_not_the_sequence_of_its_formal_is_refused, idl_7_4_12_4_2_a_sequence_formal_over_an_anonymous_element_takes_the_compliant_sequence}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113, `-p zerodds-idl-lower -p zerodds-idl-compose -p zerodds-ccm -p zerodds-idl-parse --all-targets` 726 of 726. The file goes through `idl-compose` (the `#include`), `idl-parse` and `idl-lower`. Checked: every type, constant, exception, interface and connector base of the part without a data type with its members, operations, parameters and `raises`; `module CCM_DDS::Typed<Foo, FooSeq> Foo_conn;` gives the seven interfaces with `Foo` and `FooSeq` in place of `T` and `TSeq`, the six port types and the connectors `DDS_State` and `DDS_Event` with the ports and attributes of the annex; a component with a port of such a port type gets its basic ports, `DDS_State` their mirror (equivalent IDL of `crates/ccm`); the traits of `ccm_dds` and `ccm_dds_typed` have the operations and attributes of the IDL and no other, each with the parameters of the IDL (name, order, direction: `in` by value or `&`, `inout` and `out` as `&mut`; type by the mapping of the module documentation), the result type and the exceptions (one as its structure, more as `UpdaterError` or `ReaderError`, whose variants are the exceptions of the operations of the interface), an attribute as a getter and, unless `readonly`, as a setter with its `setraises`, the data model of `crates/ccm` names the same port types, basic ports, connectors and attributes. The Rust traits are written by hand and checked against the IDL, not generated from it. Not checked: the file with an IDL compiler of another vendor. omniidl from omniORB 4.3.3 is installed on the Linux test host and refuses the IDL3+ of the annex: `omniidl -bcxx ccm_dds.idl` ends with `ccm_dds.idl:143: Syntax error in definition` (line 143 is `connector DDS_Base {`) and further syntax errors on lines 144 to 148. `tao_idl` is not installed on the Linux test host.

---

## Annex B — IDL for DDS-DLRL ports and connectors

**Spec:** Annex B, p. 41 (PDF, normative) — `local interface CacheOperation` in
module `CCM_DDS`, `#include "dds_rtf2_dlrl.idl"`.

**Repo:** `crates/dds4ccm/idl/ccm_dds_dlrl.idl` (the text of the annex);
`crates/dds4ccm/idl/dds_rtf2_dlrl.idl` (includes the generic DLRL IDL of
DDS 1.2 §8.2.1.2.1, `crates/dlrl/idl/dds_dlrl.idl`, which must be on the
include path, as the IDL directory of a DDS installation is; before it
`dds_rtf2_dcps.idl`, and the guard `DDS_DCPS_IDL` makes its declarations of
`StringSeq`, `Entity`, `DomainParticipant`, `Publisher` and `Subscriber` the
only ones, so that Annex A and Annex B compile in one unit);
`crates/dds4ccm/idl/dds_rtf2_dcps.idl` (with `DomainParticipant`, `Publisher`
and `Subscriber`, which `DDS::Cache` and `DDS::CacheDescription` refer to);
the Rust mapping `crates/dds4ccm/src/dlrl.rs::CacheOperation` (the mapping is
in the documentation of the trait); `ci/integration_scenarios.py` (job
`integration-bridges-vendor-corba`, scenario `dds4ccm-idl-omniidl`).

**Tests:** `crates/dds4ccm/tests/annex_idl.rs::{dds4ccm_annex_b_cache_operation_compiles_to_the_local_interface_of_the_annex, dds4ccm_annex_b_cache_operation_retains_attributes_and_operations_of_dds_cache_unchanged, dds4ccm_annex_b_compiles_with_annex_a_in_one_unit, dds4ccm_annex_b_the_rust_cache_operation_has_the_operations_of_the_idl}`, `crates/dds4ccm/tests/omniidl.rs::{dds4ccm_annex_b_omniidl_compiles_the_idl_of_the_annex, dds4ccm_annex_b_omniidl_refuses_the_annex_without_the_dlrl_idl}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 131 of 131; `cargo nextest run --profile merge -p zerodds-dds4ccm --test omniidl --run-ignored only` 2 of 2 with the omniidl of omniORB 4.3.3 (Debian package `omniidl`). The file goes through `idl-compose` (the `#include`s), `idl-parse` and `idl-lower`. Checked: `CCM_DDS::CacheOperation` is a `local interface` without base with the six `readonly` attributes and the seven operations of the annex, each with its parameters (direction, type, name), result and `raises`; the types are those of the generic DLRL IDL (`DDS::CacheUsage` with `READ_ONLY`, `WRITE_ONLY`, `READ_WRITE`, the four sequences of `ObjectRoot`, `ObjectHome`, `CacheAccess`, `CacheListener`, the exceptions with `string message`). "All the retained functions mimic exactly the DDS::Cache ones" (§8.2.1.1): each attribute and operation of `CacheOperation` is declared exactly once and the same in `DDS::Cache` or its base `DDS::CacheBase`; left out are the set-up of the Cache that the fragment does (`register_home`, `register_all_for_pubsub`, `enable_all_for_pubsub`, `pubsub_state`, `the_publisher`, `the_subscriber`, `find_home_by_name`, `find_home_by_index`), `kind`, `load`, `lock` and `unlock`. Annex A and Annex B compile together in either order; without the guard the two declarations of `DDS::StringSeq` and `DDS::Entity` collide, without the DLRL IDL on the include path Annex B does not compile. The trait `CacheOperation` has the attributes and operations of the IDL and no other, with the parameters of the IDL (name, order, type by the mapping) and the result; each method has the signature of the method of `zerodds_dlrl::cache::Cache` of the same name; `DCPSError`, `PreconditionNotMet` and `AlreadyDeleted` are variants of `DlrlException` with the repository id of the exception of the DLRL IDL. omniidl compiles the file with its C++ back end; the class `CacheOperation` has each attribute and operation, and without the DLRL IDL on the include path omniidl refuses the file. §8.2.1.1 prints `void DDS::refresh( )`, Annex B `void refresh( )`; the file follows the annex, since an operation name is an identifier (IDL 4.2 §7.4.3.4.3.3.1, rule 82). The former data model (`DlrlPort`, `ConnectorPattern::Dlrl` in `crates/ccm/src/dds4ccm.rs`) is removed.

---

## Annex C — XML Schema for QoS Profiles

**Spec:** Annex C, p. 43-50 (PDF, normative) — the XML schema of the QoS
profiles: namespace `http://www.omg.org/dds/`, the simple types, one complex
type per policy, the six QoS containers, `qosProfile` with `name`,
`base_name` and `topic_filter`.

**Repo:** `crates/xml/schemas/dds4ccm_qos_profile.xsd` (the type definitions of
the annex; with the global element `dds` added, the root of §7.4.2.1, since a
schema without a global element has no valid document),
`crates/xml/src/schemas.rs::{DDS4CCM_QOS_PROFILE_XSD, DDS4CCM_QOS_PROFILE_NAMESPACE}`;
the reader `crates/xml/src/qos_parser.rs`.

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_c_the_schema_file_has_the_types_of_the_annex_and_the_root_dds, dds4ccm_annex_c_what_the_schema_accepts_we_read, dds4ccm_annex_c_what_the_schema_rejects_we_reject, dds4ccm_7_4_2_the_forms_of_the_text_the_schema_rejects_have_their_values, dds4ccm_7_4_2_2_2_octets_are_the_list_of_the_text_or_the_base64_of_the_schema}`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/dds4ccm/tests/pattern_profiles.rs::dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548; `xmllint` (libxml2-utils) is the comparison. What the schema accepts, the reader reads, with the value of each field. What it rejects, the reader rejects, except where the text of §7.4.2 writes otherwise; each such place is in the test with its clause (see §7.4.2.2 and §7.4.2.3): elements without a namespace, `topic_filter` with `*`, names with `_` and `:`, `DURATION_ZERO_SEC`/`DURATION_ZERO_NSEC`, a hexadecimal `history/depth` and `resource_limits/max_samples`, the Booleans `yes` and `TRUE`, a negative `transport_priority/value`, `<participant_qos>`, octets as a list, `<qos_library>` (the form of DDS-XML 1.0). Likewise by §7.4.2.1 Table 7.1: the Booleans `yes` and `TRUE`, a hexadecimal number in `resource_limits`, a negative `transport_priority/value`. The six types `…QosProfile` restrict their base type without a content model and so have empty content by XML Schema Part 1, 3.4.2; the element `dds` therefore declares the single QoS of §7.4.2.3.3 with the base type (head of the file). In CI the comparisons with `xmllint` return unchecked (`[skip]`) until the image `ci-rust` is rebuilt from `ci/Dockerfile.rust`.

---

## Annex D — Default QoS Profile

**Spec:** Annex D, p. 51-55 (PDF, non-normative) — a QoS profile "with all
default values as specified in DDS", with every policy of the six containers.

**Repo:** `crates/xml/schemas/dds4ccm_default_qos_profile.xml` (the document of
the annex), `crates/xml/src/schemas.rs::DDS4CCM_DEFAULT_QOS_PROFILE_XML`.

**Tests:** `crates/xml/tests/dds4ccm_profiles.rs::{dds4ccm_annex_d_the_default_profile_is_valid_against_the_schema_of_annex_c, dds4ccm_annex_d_the_default_profile_reads_to_the_dds_defaults}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-xml --all-targets` 548 of 548. The document is valid against the schema of Annex C (`xmllint`); every value the reader makes of it is the default of `zerodds_qos`. The name of the profile is `" DDS DefaultQosProfile"`, with the leading space of the annex. In CI the comparisons with `xmllint` return unchecked (`[skip]`) until the image `ci-rust` is rebuilt from `ci/Dockerfile.rust`.

---

## Annex E — QoS Policies for the DDS Patterns

**Spec:** Annex E, p. 57-61 (PDF, non-normative) — four tables, one per role
(observable and observer of the state pattern, supplier and consumer of the
event pattern), with the policies of Topic, DataReader, DataWriter, Subscriber
and Publisher and whether the pattern requires the value.

**Repo:** `crates/dds4ccm/src/profiles.rs::{STATE_TRANSFER, EVENT_TRANSFER}`,
`crates/dds4ccm/profiles/state_transfer.xml`,
`crates/dds4ccm/profiles/event_transfer.xml`.

**Tests:** `crates/dds4ccm/tests/pattern_profiles.rs::{dds4ccm_annex_e_the_state_profile_has_the_values_of_the_observable_and_the_observer_table, dds4ccm_annex_e_the_event_profile_has_the_values_of_the_supplier_and_the_consumer_table, dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c, dds4ccm_annex_e_a_state_connector_with_the_state_profile_gives_a_late_observer_the_last_value, dds4ccm_annex_e_an_event_connector_with_the_event_profile_delivers_what_is_sent_from_then_on}`.

**Status:** done — green on the Linux test host: `cargo nextest run --profile merge -p zerodds-dds4ccm --all-targets` 113 of 113. A connector has one `qos_profile` for all its ports; the two tables of a pattern are therefore one profile: DataWriter and Publisher of the writing role, DataReader and Subscriber of the reading role, and the Topic, for which both tables give the same values. State: `TRANSIENT_LOCAL`, `RELIABLE`, `KEEP_LAST` with depth 1, `BY_SOURCE_TIMESTAMP`, PRESENTATION `INSTANCE` with `ordered_access`. Event: `VOLATILE`, `BEST_EFFORT`, `KEEP_ALL`, `BY_SOURCE_TIMESTAMP`, PRESENTATION `INSTANCE` with `ordered_access`, `autodispose_unregistered_instances` FALSE. The tests read the values off the profile and over DDS: a late observer of a `DDS_State` with the state profile gets the last value, a `DDS_Event` with the event profile delivers what is sent from then on. Decision: the connector does not apply the profiles by itself; without a `qos_profile` its entities have the default QoS of DDS and the HISTORY of its pattern (§7.3.2, §7.3.3). Reason: PRESENTATION and DESTINATION_ORDER are compared between DataWriter and DataReader, and a default Publisher or Subscriber of a container suits only with the same PRESENTATION (§7.4.3); a connector with these values as its default would match a DDS application with the default QoS in one direction only. In CI the comparison with `xmllint` (`dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c`) returns unchecked (`[skip]`) until the image `ci-rust` is rebuilt from `ci/Dockerfile.rust`.

---

## Audit status

21 done / 4 partial / 0 open / 0 blocked / 10 n/a (informative).

Test run: the survey from CI (`python3 ci/coverage_results.py --document dds4ccm-1.1.md`, pipelines 8501 to 9053 of `main`) is older than the tests this document now cites (`crates/dds4ccm`, `crates/idl-lower`, `crates/xml/tests/dds4ccm_profiles.rs`); its numbers no longer hold and are therefore not given here. It is taken again after the merge. Until then the results from the Linux test host are in the status lines of the items.

Open items: `dds4ccm-1.1.open.en.md`.
