# DDS-OPC UA Gateway 1.0 — Spec-Coverage

**Spec:** [OMG DDS-OPC UA Gateway 1.0 — formal/2020-01-01 →](https://www.omg.org/spec/DDS-OPCUA/)

**Kontext:** DDS-OPC-UA-Gateway bridge zwischen DDS-Topics und
OPC-UA-AddressSpace (IEC 62541). ZeroDDS implementiert die
**Type-System-Mapping**-Layer (§8.2 + §9.2) sowie das
**AddressSpace-Mapping** (§9.3) als pure-Rust no_std+alloc Library. Subscription-Behavior + OPC-UA-Binary-
Wire-Encoding sind Caller-Layer (typisch externer OPC-UA-Stack).

Implementation:

- `crates/opcua-gateway/` — Type-System- + AddressSpace-Mapping, 10 Module, 133 Tests grün.

---

## §1 Scope

### §1 Bidirectional Bridge OPC UA ↔ DDS

**Spec:** §1, S. 1 (PDF) — "This specification overcomes this
situation by defining a standard, vendor-independent, configurable
gateway that enables interoperability and information exchange between
systems that use DDS and systems that use OPC UA."

**Repo:** `crates/opcua-gateway/src/lib.rs` Crate-Doc.

**Tests:** die beiden Richtungen, die das Zitat als Brücke benennt —
OPC UA → DDS:
`crates/opcua-gateway/src/types.rs::tests::primitive_mapping_matches_spec_table_8_1`,
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::builtin_scalar_mappings_match_spec`;
DDS → OPC UA:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::primitive_dds_to_opc_ua_names_match_spec_tab_92`,
`crates/opcua-gateway/src/address_space.rs::tests::topic_node_carries_parent_domain_reference`;
dass beide Richtungen zugleich deklariert sind:
`crates/opcua-gateway/src/conformance.rs::tests::declares_all_four_conformance_points`.

**Status:** done

---

## §2 Conformance

### §2 Conformance Point 1: OPC UA to DDS Mapping Basic

**Spec:** §2 Punkt 1 (S. 1) — UA Type-System + UA Subscription
Mapping.

**Repo:** Type-System-Mapping in
`crates/opcua-gateway/src/types.rs` + `dds_to_ua/walker.rs`;
Subscription-Mapping in
`crates/opcua-gateway/src/subscription_mapping/{behavior,config,
variant_dds}.rs`.

**Tests:** UA-Type-System:
`crates/opcua-gateway/src/types.rs::tests::{builtin_type_kind_values_match_spec,
primitive_mapping_matches_spec_table_8_1, xtypes_names_match_spec_table_8_1}`;
UA-Subscription-Modell:
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::{primitive_scalar_mappings_match_spec_tab_816,
builtin_scalar_mappings_match_spec, classify_scalar_array_matrix,
array_1d_wraps_in_sequence, matrix_returns_named_struct,
all_25_builtins_have_matrix_alias}`;
`crates/opcua-gateway/src/conformance.rs::tests::points_are_numbered_one_to_four_uniquely`.

**Status:** done

### §2 Conformance Point 2: OPC UA to DDS Mapping Complete

**Spec:** §2 Punkt 2 — Basic + UA Service-Sets.

**Repo:** Punkt 1 + Service-Sets in
`crates/opcua-gateway/src/service_sets/{attribute, method, query,
view}.rs`.

**Tests:** `crates/opcua-gateway/src/service_sets/mod.rs::tests::{idl_module_paths_match_spec,
all_service_sets_covers_four_sets, filter_operator_values_match_spec,
timestamps_to_return_values_match_spec,
filter_operand_kind_discriminant_roundtrip}`;
`crates/opcua-gateway/src/conformance.rs::tests::complete_points_subsume_their_basic`.

**Status:** done

### §2 Conformance Point 3: DDS to OPC UA Mapping Basic

**Spec:** §2 Punkt 3 — DDS Type-System + DDS Global Data Space
(without Historical).

**Repo:** Type-System via `dds_to_ua/walker.rs` (siehe §9.2);
GDS-Mapping in `crates/opcua-gateway/src/address_space.rs::{DomainNode, TopicNode,
SampleVariable}` (siehe §9.3).

**Tests:** DDS-Type-System:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::{struct_emits_data_type_and_variable_type,
enum_emits_enumeration_subtype, union_emits_union_subtype_with_switchfield,
primitive_dds_to_opc_ua_names_match_spec_tab_92}`;
DDS-Global-Data-Space:
`crates/opcua-gateway/src/address_space.rs::tests::{domain_node_for_zero_yields_browse_name_domain0,
topic_node_carries_parent_domain_reference, scalar_topic_sample_is_a_leaf}`.

**Status:** done

### §2 Conformance Point 4: DDS to OPC UA Mapping Complete

**Spec:** §2 Punkt 4 — Basic + Historical Access.

**Repo:** Punkt 3 + Historical-Access in
`crates/opcua-gateway/src/historical/{adapter, config, qos}.rs`
mit DDS-DurabilityService → UA-HistoryRead-Mapping.

**Tests:** `crates/opcua-gateway/src/historical/adapter.rs::tests::{raw_read_returns_samples_in_range,
at_time_returns_nearest_prior_sample}`;
`crates/opcua-gateway/src/historical/config.rs::tests::{historical_default_has_history_and_current_read_bits,
history_read_bit_value_matches_spec}`;
`crates/opcua-gateway/src/conformance.rs::tests::complete_points_subsume_their_basic`.

**Status:** done

### §2 Multi-Point Conformance Marker

**Spec:** §2 — eine Implementierung kann mehrere Conformance-Points
gleichzeitig erfüllen ("multi-point").

**Repo:** `crates/opcua-gateway/src/conformance.rs` — expliziter,
maschinen-abfragbarer Marker: `ConformancePoint`-Enum (alle vier Punkte
mit Titel + Nummer + `subsumes_basic`), `DECLARED_CONFORMANCE` (= alle
vier) + `is_multi_point_conformant()` / `declares()`. Damit kann ein
Server-Stack-Integrator den Conformance-Stand programmatisch abfragen
statt Prosa zu lesen.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::{declares_all_four_conformance_points,
is_multi_point, points_are_numbered_one_to_four_uniquely,
complete_points_subsume_their_basic, every_point_has_a_distinct_title}`.

**Status:** done

---

## §3 Normative References

### §3 References (DDS/DDSI/XTypes/IDL/OPCUA)

**Spec:** §3, S. 2-3 — DDS 1.4, DDS-RPC 1.0, DDS-Security 1.1,
DDS-WEB, DDS-XML, DDS-XTYPES 1.2, DDSI-RTPS 2.3, IDL 4.2,
OPCUA-01..-12.

**Repo:** Cross-Ref alle eigenen Spec-Coverage-Files
(`zerodds-dcps-1.4.md`, `ddsi-rtps-2.5.md`, `dds-xtypes-1.3.md`,
`omg-time-1.1.md`, `idl-4.2.md`, etc.).

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz-Liste; OPCUA-/DDS-Specs werden in den Konsumenten-Items §8/§9 operativ erfüllt.

---

## §4-§7 Terminologie + informative Abschnitte

### §4 Terms and Definitions

**Spec:** §4 (PDF) — Glossar.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar; ohne Code-Mapping.

### §5 Symbols and Abbreviations

**Spec:** §5 (PDF) — Abkürzungen (DDS, OPC-UA, GDS, etc.).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Symbol-Tabelle; ohne Code-Mapping.

---

### §6 Additional Information

**Spec:** §6 (PDF) — informativer Anhang (Roadmap, Acknowledgements).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — §6.1 Acknowledgements; keine Anforderung.

### §7 UA/DDS Overview (Informative)

**Spec:** §7 (PDF) — non-normative Overview-Material zu UA + DDS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert §7 explizit als
non-normative Overview.

---

## §8 OPC UA to DDS Bridge

### §8.1 OPC UA to DDS Bridge Overview

**Spec:** §8.1 (PDF) — Bridge-Architektur-Überblick (informativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — in der Spec als "Overview (non-normative)" gekennzeichnet; die Anforderungen stehen in §8.2–§8.4.

### §8.2 OPC UA Type System Mapping

**Spec:** §8.2, S. 11-12 (PDF) — `BuiltinTypeKind` Enum mit allen 25
OPC-UA-Built-in-Type-IDs (Boolean=1, ..., DiagnosticInfo=25).

**Repo:** `crates/opcua-gateway/src/types.rs::BuiltinTypeKind` mit
allen 25 Werten + `value()`-Methode.

**Tests:** `crates/opcua-gateway/src/types.rs::tests::builtin_type_kind_values_match_spec`,
`crates/opcua-gateway/src/types.rs::tests::node_class_values_match_bit_flags`.

**Status:** done

### §8.2.1 Built-in Primitive Types

**Spec:** §8.2.1, S. 12 Tab 8.1 — Primitive-Mapping (Boolean→Boolean,
SByte→Byte/int8, Byte→Byte/uint8, Int16/UInt16/Int32/UInt32/Int64/
UInt64 1:1, Float→Float32, Double→Float64, String→String8).

**Repo:** `crates/opcua-gateway/src/types.rs::{dds_idl_primitive,
dds_xtypes_name, map_primitive_to_dds}`.

**Tests:** `types::tests::primitive_mapping_matches_spec_table_8_1`,
`xtypes_names_match_spec_table_8_1`,
`sbyte_and_byte_both_map_to_dds_byte_in_xtypes`,
`composite_types_have_no_primitive_mapping`,
`map_primitive_to_dds_helper_yields_token`.

**Status:** done

### §8.2.2 Built-in Complex Types

**Spec:** §8.2.2, S. 13-15 Tab 8.2 — DateTime/Guid/ByteString/
XmlElement/NodeId/ExpandedNodeId/StatusCode/QualifiedName/
LocalizedText/ExtensionObject/DataValue/Variant/DiagnosticInfo
mit den exakten IDL-Equivalent-Strukturen.

**Repo:** Voll modelliert in:
* `crates/opcua-gateway/src/types.rs::{Guid, StatusCode, ByteString, QualifiedName, LocalizedText}`.
* `node_id.rs::{NodeId, NodeIdentifier, NodeIdentifierKind,
  ExpandedNodeId}` mit allen 4 Identifier-Cases (Numeric/String/Guid/
  Opaque) + 4096-Byte-Limit-Validation.
* `data_value.rs::{Variant, VariantValue, DataValue, ExtensionObject,
  ExtensionObjectBody, BodyEncoding}` mit allen 21 VariantValue-Cases.

**Tests:** `types::tests::guid_round_trips_via_bytes`,
`qualified_name_carries_namespace_and_name`,
`localized_text_supports_optional_fields`;
`node_id::tests::*` (8 Tests inkl. canonical-string-Format
`ns=N;<kind>=<id>`);
`data_value::tests::*` (8 Tests inkl. round-trip aller VariantValue-
Cases + scalar/1d_array/multi-dim Detection).

**Status:** done — `DiagnosticInfo` ist als ScopedName-Bezugspunkt
exposed; das mutable @optional-Felder-Modell wäre analog zu
`LocalizedText` ergänzbar.

### §8.3 OPC UA Service Sets Mapping

**Spec:** §8.3, S. 16-30 — die vier Service-Sets View, Query, Attribute
und Method werden als DDS-RPC-Interfaces abgebildet; die Spec gibt je
Set die Methoden-Liste (View 5, Query 2, Attribute 4, Method 1), die
IDL-Modulpfade und die Enum-Werte für `BrowseDirection`,
`TimestampsToReturn` und die Filter-Operatoren vor.

**Repo:** `crates/opcua-gateway/src/service_sets/{view,query,attribute,
method}.rs` mit DDS-RPC-Bindings über `crates/rpc/`; die gemeinsamen
Enums und Modulpfade in `crates/opcua-gateway/src/service_sets/mod.rs`.

**Tests:** Methodenzahl je Set:
`crates/opcua-gateway/src/service_sets/view.rs::tests::view_set_has_five_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/query.rs::tests::query_set_has_two_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/attribute.rs::tests::attribute_set_has_four_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/method.rs::tests::method_set_has_one_method_per_spec`;
Signaturen und Enum-Werte:
`crates/opcua-gateway/src/service_sets/view.rs::tests::{browse_method_first_param_is_server_id,
browse_method_carries_view_description,
browse_direction_uses_spec_values_with_gap, all_methods_return_response_header}`,
`crates/opcua-gateway/src/service_sets/query.rs::tests::{query_first_carries_content_filter_inputs,
query_next_uses_release_continuation_flag}`,
`crates/opcua-gateway/src/service_sets/attribute.rs::tests::{read_carries_timestamps_to_return,
history_update_uses_extensible_details_sequence, history_read_details_kind_discriminant,
history_update_details_kind_discriminant, perform_update_type_values_match_spec,
history_update_type_values_match_spec}`,
`crates/opcua-gateway/src/service_sets/method.rs::tests::{call_carries_methods_to_call_input,
call_method_request_holds_node_ids_and_variants}`,
`crates/opcua-gateway/src/service_sets/mod.rs::tests::{idl_module_paths_match_spec,
all_service_sets_covers_four_sets, filter_operator_values_match_spec,
timestamps_to_return_values_match_spec,
filter_operand_kind_discriminant_roundtrip}`.

**Status:** done

### §8.4 OPC UA Subscription Model Mapping

**Spec:** §8.4, S. 31-48 — OPC-UA-Subscriptions werden auf
DDS-DataReader-Listener abgebildet: ein MonitoredItem trägt genau einen
Filter, die Subscription-Lifetime ist an das Publishing-Intervall
gebunden, und Tab 8.16 gibt die Variant-nach-DDS-Typ-Zuordnung für
Skalar, 1D-Array und Matrix vor.

**Repo:** `crates/opcua-gateway/src/subscription_mapping/{behavior,
config,variant_dds}.rs`.

**Tests:** Konfigurations-Constraints:
`crates/opcua-gateway/src/subscription_mapping/config.rs::tests::{data_item_with_two_filters_is_invalid,
data_item_with_one_filter_is_valid, subscription_protocol_lifetime_constraint,
monitored_item_kind_discriminants, assignment_input_kind_discriminants}`;
Push-Verhalten:
`crates/opcua-gateway/src/subscription_mapping/behavior.rs::tests::{constant_assignment_writes_single_field,
data_change_finds_and_assigns_value, data_change_unknown_handle_is_error,
event_field_assignment_picks_correct_index,
event_field_index_out_of_range_is_error, build_client_handles_indexes_correctly}`;
Variant→DDS-Typ (Tab 8.16):
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::{classify_scalar_array_matrix,
primitive_scalar_mappings_match_spec_tab_816, builtin_scalar_mappings_match_spec,
array_1d_wraps_in_sequence, matrix_returns_named_struct,
all_25_builtins_have_matrix_alias}`.

**Status:** done

---

## §9 DDS to OPC UA Bridge

### §9.1 DDS to OPC UA Bridge Overview

**Spec:** §9.1 (PDF) — Bridge-Architektur DDS→UA (informativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — in der Spec als "Overview (non-normative)" gekennzeichnet; die Anforderungen stehen in §9.2–§9.3.

### §9.2 DDS Type System Mapping

**Spec:** §9.2, S. 49-93 — DDS-Primitive/String/Enum/Aggregated/
Collection/Nested/Alias/Keyed-Types → OPC-UA-Variables.

**Repo:** Vollständiges DDS→UA-Type-Walking in
`crates/opcua-gateway/src/dds_to_ua/walker.rs::{walk_dds_type, DdsType,
WalkOutput}` und `crates/opcua-gateway/src/dds_to_ua/node_spec.rs::NodeSpec` mit rekursiver Aggregated- (Struct/Union) und Collection-
Behandlung (Sequence-of-Struct, Map<String, Sequence<Struct>>);
inverses Mapping symmetrisch via `BuiltinTypeKind`. **Instance-Seite:**
`crates/opcua-gateway/src/address_space.rs::build_sample_instance` zerlegt ein strukturiertes
Sample rekursiv in eine `InstanceNode`-Variablen-Hierarchie
(HasComponent-Komponenten pro Member, verschachtelt; Collections als
Array-/Object-Variablen) — spiegelt die §9.2-Typ-Rekursion auf der
§9.3-Instance-Seite. `SampleVariable::scalar` bleibt für den einfachen
Skalar-Wert-Pfad.

**Tests:** Typ-Walker:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::{struct_emits_data_type_and_variable_type,
struct_dt_subtypes_structure, nested_struct_recurses,
enum_emits_enumeration_subtype, bitmask_emits_optionset_subtype,
union_emits_union_subtype_with_switchfield,
union_with_invalid_discriminator_is_rejected, array_of_struct_recurses_into_struct,
sequence_of_struct_recurses, map_recurses_into_key_and_value,
alias_is_transparent, member_modeling_rule_reflects_optional,
primitive_dds_to_opc_ua_names_match_spec_tab_92,
alias_resolve_unwraps_recursively}` (14);
Namensbildung:
`crates/opcua-gateway/src/dds_to_ua/naming.rs::tests::{struct_data_type_appends_suffix,
struct_variable_type_appends_suffix, array_element_single_dim_uses_underscore,
array_element_multi_dim_concatenates_with_underscore,
array_element_zero_dim_returns_bare_name, undefined_position_uses_decimal_index}`;
Instanz-Builder:
`crates/opcua-gateway/src/address_space.rs::tests::{struct_sample_decomposes_into_component_variables,
nested_struct_sample_recurses_into_components, array_member_is_an_array_variable,
sequence_member_is_a_one_dim_array_variable,
sequence_of_struct_is_an_array_not_decomposed,
union_sample_has_switchfield_and_case_components,
key_and_optional_members_surface_on_instance,
alias_member_is_transparent_on_instance}`.

**Status:** done — Datenmodell + Scalar-Mapping + Aggregated/Collection/
Nested-Recursion im **Typ-Walker UND im Instance-Builder** abgedeckt.

### §9.3 DDS Global Data Space Mapping

**Spec:** §9.3, S. 94-107 — DDS-Domain → OPC-UA-ObjectNode mit
BrowseName `Domain<id>`; DDS-Topic → OPC-UA-ObjectNode mit BrowseName
= Topic-Name; DDS-Sample → OPC-UA-Variable.

**Repo:** `crates/opcua-gateway/src/address_space.rs::{DomainNode,
TopicNode, SampleVariable, mangle_topic_node_browse_name}`.

**Tests:** `address_space::tests::domain_node_for_zero_yields_browse_name_domain0`,
`domain_node_for_42_yields_browse_name_domain42`,
`mangle_topic_node_browse_name_is_pass_through`,
`topic_node_carries_parent_domain_reference`,
`sample_variable_scalar_has_value_rank_minus_1`.

**Status:** done

### §9.3.4 Reading Historical Data from Instance Nodes

**Spec:** §9.3.4 — Historical-Access auf Instance-Nodes
(Conformance-Point 4 "Complete"): ein Instance-Node ist nur dann
historisierend, wenn `Historizing` gesetzt und das `HistoryRead`-Bit
(Wert 4) im `AccessLevel` vorhanden ist; die DataReader-History-QoS muss
tief genug sein; `ReadRawModified` liefert die Samples eines
Zeitintervalls, `ReadAtTime` das jeweils zeitlich vorangehende Sample.

**Repo:** `crates/opcua-gateway/src/historical/{adapter,config,
qos}.rs`.

**Tests:** AccessLevel + Historizing:
`crates/opcua-gateway/src/historical/config.rs::tests::{historical_default_has_history_and_current_read_bits,
historizing_false_fails_validation, missing_history_read_bit_fails_validation,
history_read_bit_value_matches_spec}`;
History-QoS-Tiefe:
`crates/opcua-gateway/src/historical/qos.rs::tests::{keep_all_always_valid,
keep_last_with_sufficient_depth_valid, keep_last_below_minimum_violates}`;
Lese-Pfade:
`crates/opcua-gateway/src/historical/adapter.rs::tests::{raw_read_returns_samples_in_range,
raw_read_truncates_at_num_values_per_node, raw_read_invalid_range_errors,
at_time_returns_nearest_prior_sample, at_time_empty_req_times_errors,
unknown_instance_errors}`.

**Status:** done

---

## §10 OPC UA/DDS Gateway Configuration

### §10.1 Gateway Configuration Overview

**Spec:** §10.1, S. 109 — "The syntax to configure the OPC UA/DDS Gateway is specified in two normative XSD files": `dds-opcua_definitions_nonamespace.xsd` und `dds-opcua_configuration.xsd` (Wurzelelement, Namespace `http://www.omg.org/spec/DDS-OPCUA`); "This is the schema file that shall be used to validate OPC UA/DDS Gateway XML configuration files."

**Repo:** —

**Tests:** —

**Status:** open — im Baum liegen die beiden XSD-Dateien nicht; `crates/opcua-gateway/src/xml.rs` validiert gegen kein Schema und liest eine eigene Syntax (Item §10.2).

### §10.2 XML Configuration

**Spec:** §10.2, S. 109–113, Table 10.1 — Konfigurationselemente: Wurzel `<dds>` (`rootType`) mit `<types>`, `<qos_libraries>` und `<ddsopcua_gateway>`, darin `<opcua_connection>`, `<domain_participant>`, `<opcua_to_dds_bridge>` (`<service_set>`, `<subscription>`, `<opcua_input>`, `<dds_output>`, `<mapping>`) und `<dds_to_opcua_bridge>` (`<domain>`, `<topic>`, `<topic_group>`); Attribute und Details nach der normativen XSD.

**Repo:** —

**Tests:** —

**Status:** open — `crates/opcua-gateway/src/xml.rs` (`parse_gateway_config`) liest eine eigene Syntax mit Wurzel `<zerodds_opcua_gateway>`, `<bridge>`, `<ua_to_dds_connection>` und `<dds_to_ua_connection>` und weist die Wurzel `<dds>` der Spec ab; die Tests in `xml.rs` prüfen diese eigene Syntax.

---

## Audit-Status

14 done / 0 partial / 2 open / 0 blocked / 7 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document dds-opcua-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-gateway --all-targets` (Job `merge-rust-opcua-gateway`) — 102 Tests grün.

102 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `dds-opcua-1.0.open.md`.
