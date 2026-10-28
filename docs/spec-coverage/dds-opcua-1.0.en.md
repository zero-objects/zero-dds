# DDS-OPC UA Gateway 1.0 — Spec Coverage

**Spec:** [OMG DDS-OPC UA Gateway 1.0 — formal/2020-01-01 →](https://www.omg.org/spec/DDS-OPCUA/)

**Context:** the DDS-OPC-UA gateway bridges between DDS topics and the
OPC-UA address space (IEC 62541). ZeroDDS implements the **type-system
mapping** layer (§8.2 + §9.2) and the **address-space mapping** (§9.3) as a
pure-Rust no_std+alloc library. Subscription
behavior + the OPC-UA binary wire encoding are caller-layer (typically an
external OPC-UA stack).

Implementation:

- `crates/opcua-gateway/` — type-system + address-space mapping, 10 modules, 133 tests green.

---

## §1 Scope

### §1 Bidirectional bridge OPC UA ↔ DDS

**Spec:** §1, p. 1 (PDF) — "This specification overcomes this situation by
defining a standard, vendor-independent, configurable gateway that enables
interoperability and information exchange between systems that use DDS and
systems that use OPC UA."

**Repo:** `crates/opcua-gateway/src/lib.rs` crate doc.

**Tests:** the two directions the quote names as the bridge — OPC UA to DDS:
`crates/opcua-gateway/src/types.rs::tests::primitive_mapping_matches_spec_table_8_1`,
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::builtin_scalar_mappings_match_spec`;
DDS to OPC UA:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::primitive_dds_to_opc_ua_names_match_spec_tab_92`,
`crates/opcua-gateway/src/address_space.rs::tests::topic_node_carries_parent_domain_reference`;
that both directions are declared at once:
`crates/opcua-gateway/src/conformance.rs::tests::declares_all_four_conformance_points`.

**Status:** done

---

## §2 Conformance

### §2 Conformance point 1: OPC UA to DDS mapping basic

**Spec:** §2 point 1 (p. 1) — UA type system + UA subscription mapping.

**Repo:** type-system mapping in `crates/opcua-gateway/src/types.rs` +
`dds_to_ua/walker.rs`; subscription mapping in
`crates/opcua-gateway/src/subscription_mapping/{behavior,config,variant_dds}.rs`.

**Tests:** UA type system:
`crates/opcua-gateway/src/types.rs::tests::{builtin_type_kind_values_match_spec,
primitive_mapping_matches_spec_table_8_1, xtypes_names_match_spec_table_8_1}`;
UA subscription model:
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::{primitive_scalar_mappings_match_spec_tab_816,
builtin_scalar_mappings_match_spec, classify_scalar_array_matrix,
array_1d_wraps_in_sequence, matrix_returns_named_struct,
all_25_builtins_have_matrix_alias}`;
`crates/opcua-gateway/src/conformance.rs::tests::points_are_numbered_one_to_four_uniquely`.

**Status:** done

### §2 Conformance point 2: OPC UA to DDS mapping complete

**Spec:** §2 point 2 — basic + UA service sets.

**Repo:** point 1 + service sets in
`crates/opcua-gateway/src/service_sets/{attribute, method, query, view}.rs`.

**Tests:** `crates/opcua-gateway/src/service_sets/mod.rs::tests::{idl_module_paths_match_spec,
all_service_sets_covers_four_sets, filter_operator_values_match_spec,
timestamps_to_return_values_match_spec,
filter_operand_kind_discriminant_roundtrip}`;
`crates/opcua-gateway/src/conformance.rs::tests::complete_points_subsume_their_basic`.

**Status:** done

### §2 Conformance point 3: DDS to OPC UA mapping basic

**Spec:** §2 point 3 — DDS type system + DDS global data space (without
historical).

**Repo:** type system via `dds_to_ua/walker.rs` (see §9.2); GDS mapping in
`crates/opcua-gateway/src/address_space.rs::{DomainNode, TopicNode, SampleVariable}` (see §9.3).

**Tests:** DDS type system:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::{struct_emits_data_type_and_variable_type,
enum_emits_enumeration_subtype, union_emits_union_subtype_with_switchfield,
primitive_dds_to_opc_ua_names_match_spec_tab_92}`;
DDS global data space:
`crates/opcua-gateway/src/address_space.rs::tests::{domain_node_for_zero_yields_browse_name_domain0,
topic_node_carries_parent_domain_reference, scalar_topic_sample_is_a_leaf}`.

**Status:** done

### §2 Conformance point 4: DDS to OPC UA mapping complete

**Spec:** §2 point 4 — basic + historical access.

**Repo:** point 3 + historical access in
`crates/opcua-gateway/src/historical/{adapter, config, qos}.rs` with a
DDS-DurabilityService → UA-HistoryRead mapping.

**Tests:** `crates/opcua-gateway/src/historical/adapter.rs::tests::{raw_read_returns_samples_in_range,
at_time_returns_nearest_prior_sample}`;
`crates/opcua-gateway/src/historical/config.rs::tests::{historical_default_has_history_and_current_read_bits,
history_read_bit_value_matches_spec}`;
`crates/opcua-gateway/src/conformance.rs::tests::complete_points_subsume_their_basic`.

**Status:** done

### §2 Multi-point conformance marker

**Spec:** §2 — an implementation may satisfy several conformance points at
once ("multi-point").

**Repo:** `crates/opcua-gateway/src/conformance.rs` — an explicit,
machine-queryable marker: the `ConformancePoint` enum (all four points with
title + number + `subsumes_basic`), `DECLARED_CONFORMANCE` (= all four) +
`is_multi_point_conformant()` / `declares()`. A server-stack integrator can
query the conformance status programmatically instead of reading prose.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::{declares_all_four_conformance_points,
is_multi_point, points_are_numbered_one_to_four_uniquely,
complete_points_subsume_their_basic, every_point_has_a_distinct_title}` (5).

**Status:** done

---

## §3 Normative references

### §3 References (DDS/DDSI/XTypes/IDL/OPCUA)

**Spec:** §3, p. 2-3 — DDS 1.4, DDS-RPC 1.0, DDS-Security 1.1, DDS-WEB,
DDS-XML, DDS-XTYPES 1.2, DDSI-RTPS 2.3, IDL 4.2, OPCUA-01..-12.

**Repo:** cross-ref all the own spec-coverage files (`zerodds-dcps-1.4.md`,
`ddsi-rtps-2.5.md`, `dds-xtypes-1.3.md`, `omg-time-1.1.md`, `idl-4.2.md`,
etc.).

**Tests:** —

**Status:** `n/a (informative)` — an external normative reference list; the
OPCUA/DDS specs are fulfilled operationally in the consumer items §8/§9.

---

## §4-§7 Terminology + informative sections

### §4 Terms and definitions

**Spec:** §4 (PDF) — glossary.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary; without a code mapping.

### §5 Symbols and abbreviations

**Spec:** §5 (PDF) — abbreviations (DDS, OPC-UA, GDS, etc.).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — symbol table; without a code mapping.

---

### §6 Additional information

**Spec:** §6 (PDF) — informative annex (roadmap, acknowledgements).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §7 UA/DDS overview (informative)

**Spec:** §7 (PDF) — non-normative overview material on UA + DDS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks §7 explicitly as a
non-normative overview.

---

## §8 OPC UA to DDS bridge

### §8.1 OPC UA to DDS bridge overview

**Spec:** §8.1 (PDF) — bridge architecture overview (informative).

**Repo:** cross-ref §8.2-§8.4.

**Tests:** —

**Status:** `n/a (informative)`

### §8.2 OPC UA type system mapping

**Spec:** §8.2, p. 11-12 (PDF) — a `BuiltinTypeKind` enum with all 25
OPC-UA built-in type IDs (Boolean=1, ..., DiagnosticInfo=25).

**Repo:** `crates/opcua-gateway/src/types.rs::BuiltinTypeKind` with all 25
values + a `value()` method.

**Tests:** `crates/opcua-gateway/src/types.rs::tests::builtin_type_kind_values_match_spec`,
`crates/opcua-gateway/src/types.rs::tests::node_class_values_match_bit_flags`.

**Status:** done

### §8.2.1 Built-in primitive types

**Spec:** §8.2.1, p. 12 Tab 8.1 — primitive mapping (Boolean→Boolean,
SByte→Byte/int8, Byte→Byte/uint8, Int16/UInt16/Int32/UInt32/Int64/UInt64
1:1, Float→Float32, Double→Float64, String→String8).

**Repo:** `crates/opcua-gateway/src/types.rs::{dds_idl_primitive,
dds_xtypes_name, map_primitive_to_dds}`.

**Tests:** `types::tests::primitive_mapping_matches_spec_table_8_1`,
`xtypes_names_match_spec_table_8_1`,
`sbyte_and_byte_both_map_to_dds_byte_in_xtypes`,
`composite_types_have_no_primitive_mapping`,
`map_primitive_to_dds_helper_yields_token`.

**Status:** done

### §8.2.2 Built-in complex types

**Spec:** §8.2.2, p. 13-15 Tab 8.2 — DateTime/Guid/ByteString/XmlElement/
NodeId/ExpandedNodeId/StatusCode/QualifiedName/LocalizedText/
ExtensionObject/DataValue/Variant/DiagnosticInfo with the exact
IDL-equivalent structures.

**Repo:** fully modeled in:
* `crates/opcua-gateway/src/types.rs::{Guid, StatusCode, ByteString, QualifiedName, LocalizedText}`.
* `node_id.rs::{NodeId, NodeIdentifier, NodeIdentifierKind,
  ExpandedNodeId}` with all 4 identifier cases (Numeric/String/Guid/Opaque)
  + a 4096-byte limit validation.
* `data_value.rs::{Variant, VariantValue, DataValue, ExtensionObject,
  ExtensionObjectBody, BodyEncoding}` with all 21 VariantValue cases.

**Tests:** `types::tests::guid_round_trips_via_bytes`,
`qualified_name_carries_namespace_and_name`,
`localized_text_supports_optional_fields`; `node_id::tests::*` (8 tests
incl. the canonical string format `ns=N;<kind>=<id>`);
`data_value::tests::*` (8 tests incl. round-trip of all VariantValue cases
+ scalar/1d_array/multi-dim detection).

**Status:** done — `DiagnosticInfo` is exposed as a ScopedName reference
point; the mutable @optional-field model would be extendable analogous to
`LocalizedText`.

### §8.3 OPC UA service sets mapping

**Spec:** §8.3, p. 16-30 — the four service sets View, Query, Attribute and
Method are mapped onto DDS-RPC interfaces; the spec fixes the method list per
set (View 5, Query 2, Attribute 4, Method 1), the IDL module paths and the
enum values for `BrowseDirection`, `TimestampsToReturn` and the filter
operators.

**Repo:** `crates/opcua-gateway/src/service_sets/{view,query,attribute,
method}.rs` with DDS-RPC bindings via `crates/rpc/`; the shared enums and
module paths in `crates/opcua-gateway/src/service_sets/mod.rs`.

**Tests:** method count per set:
`crates/opcua-gateway/src/service_sets/view.rs::tests::view_set_has_five_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/query.rs::tests::query_set_has_two_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/attribute.rs::tests::attribute_set_has_four_methods_per_spec`,
`crates/opcua-gateway/src/service_sets/method.rs::tests::method_set_has_one_method_per_spec`;
signatures and enum values:
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

### §8.4 OPC UA subscription model mapping

**Spec:** §8.4, p. 31-48 — OPC-UA subscriptions are mapped onto DDS
DataReader listeners: a MonitoredItem carries exactly one filter, the
subscription lifetime is bound to the publishing interval, and Table 8.16
fixes the Variant-to-DDS-type assignment for scalar, 1-D array and matrix.

**Repo:** `crates/opcua-gateway/src/subscription_mapping/{behavior,config,variant_dds}.rs`.

**Tests:** configuration constraints:
`crates/opcua-gateway/src/subscription_mapping/config.rs::tests::{data_item_with_two_filters_is_invalid,
data_item_with_one_filter_is_valid, subscription_protocol_lifetime_constraint,
monitored_item_kind_discriminants, assignment_input_kind_discriminants}`;
push behaviour:
`crates/opcua-gateway/src/subscription_mapping/behavior.rs::tests::{constant_assignment_writes_single_field,
data_change_finds_and_assigns_value, data_change_unknown_handle_is_error,
event_field_assignment_picks_correct_index,
event_field_index_out_of_range_is_error, build_client_handles_indexes_correctly}`;
Variant to DDS type (Table 8.16):
`crates/opcua-gateway/src/subscription_mapping/variant_dds.rs::tests::{classify_scalar_array_matrix,
primitive_scalar_mappings_match_spec_tab_816, builtin_scalar_mappings_match_spec,
array_1d_wraps_in_sequence, matrix_returns_named_struct,
all_25_builtins_have_matrix_alias}`.

**Status:** done

---

## §9 DDS to OPC UA bridge

### §9.1 DDS to OPC UA bridge overview

**Spec:** §9.1 (PDF) — bridge architecture DDS→UA (informative).

**Repo:** cross-ref §9.2-§9.3.

**Tests:** —

**Status:** `n/a (informative)`

### §9.2 DDS type system mapping

**Spec:** §9.2, p. 49-93 — DDS
primitive/string/enum/aggregated/collection/nested/alias/keyed types →
OPC-UA variables.

**Repo:** full DDS→UA type walking in
`crates/opcua-gateway/src/dds_to_ua/walker.rs::{walk_dds_type, DdsType,
WalkOutput}` and `crates/opcua-gateway/src/dds_to_ua/node_spec.rs::NodeSpec`
with recursive aggregated (struct/union) and collection handling
(sequence-of-struct, Map<String, Sequence<Struct>>); the inverse mapping
symmetric via `BuiltinTypeKind`. **Instance side:**
`crates/opcua-gateway/src/address_space.rs::build_sample_instance` recursively decomposes a
structured sample into an `InstanceNode` variable hierarchy (HasComponent
member components, nested; collections as array/object variables) —
mirroring the §9.2 type recursion on the §9.3 instance side.
`SampleVariable::scalar` remains for the simple scalar value path.

**Tests:** type walker:
`crates/opcua-gateway/src/dds_to_ua/walker.rs::tests::{struct_emits_data_type_and_variable_type,
struct_dt_subtypes_structure, nested_struct_recurses,
enum_emits_enumeration_subtype, bitmask_emits_optionset_subtype,
union_emits_union_subtype_with_switchfield,
union_with_invalid_discriminator_is_rejected, array_of_struct_recurses_into_struct,
sequence_of_struct_recurses, map_recurses_into_key_and_value,
alias_is_transparent, member_modeling_rule_reflects_optional,
primitive_dds_to_opc_ua_names_match_spec_tab_92,
alias_resolve_unwraps_recursively}` (14);
naming:
`crates/opcua-gateway/src/dds_to_ua/naming.rs::tests::{struct_data_type_appends_suffix,
struct_variable_type_appends_suffix, array_element_single_dim_uses_underscore,
array_element_multi_dim_concatenates_with_underscore,
array_element_zero_dim_returns_bare_name, undefined_position_uses_decimal_index}`;
instance builder:
`crates/opcua-gateway/src/address_space.rs::tests::{struct_sample_decomposes_into_component_variables,
nested_struct_sample_recurses_into_components, array_member_is_an_array_variable,
sequence_member_is_a_one_dim_array_variable,
sequence_of_struct_is_an_array_not_decomposed,
union_sample_has_switchfield_and_case_components,
key_and_optional_members_surface_on_instance,
alias_member_is_transparent_on_instance}`.

**Status:** done — data model + scalar mapping + aggregated/collection/
nested recursion in **both the type walker and the instance builder**.

### §9.3 DDS global data space mapping

**Spec:** §9.3, p. 94-107 — DDS domain → OPC-UA ObjectNode with BrowseName
`Domain<id>`; DDS topic → OPC-UA ObjectNode with BrowseName = topic name;
DDS sample → OPC-UA variable.

**Repo:** `crates/opcua-gateway/src/address_space.rs::{DomainNode,
TopicNode, SampleVariable, mangle_topic_node_browse_name}`.

**Tests:** `crates/opcua-gateway/src/address_space.rs::tests::{domain_node_for_zero_yields_browse_name_domain0,
domain_node_for_42_yields_browse_name_domain42,
mangle_topic_node_browse_name_is_pass_through,
topic_node_carries_parent_domain_reference,
sample_variable_scalar_has_value_rank_minus_1}` (5).

**Status:** done

### §9.3.4 Reading historical data from instance nodes

**Spec:** §9.3.4 — historical access on instance nodes (conformance point 4
"complete"): an instance node is historizing only when `Historizing` is set
and the `HistoryRead` bit (value 4) is present in `AccessLevel`; the
DataReader history QoS has to be deep enough; `ReadRawModified` returns the
samples of a time range, `ReadAtTime` the sample preceding each requested
time.

**Repo:** `crates/opcua-gateway/src/historical/{adapter,config,qos}.rs`.

**Tests:** AccessLevel + Historizing:
`crates/opcua-gateway/src/historical/config.rs::tests::{historical_default_has_history_and_current_read_bits,
historizing_false_fails_validation, missing_history_read_bit_fails_validation,
history_read_bit_value_matches_spec}`;
history QoS depth:
`crates/opcua-gateway/src/historical/qos.rs::tests::{keep_all_always_valid,
keep_last_with_sufficient_depth_valid, keep_last_below_minimum_violates}`;
read paths:
`crates/opcua-gateway/src/historical/adapter.rs::tests::{raw_read_returns_samples_in_range,
raw_read_truncates_at_num_values_per_node, raw_read_invalid_range_errors,
at_time_returns_nearest_prior_sample, at_time_empty_req_times_errors,
unknown_instance_errors}`.

**Status:** done

---

## §10 OPC UA/DDS gateway configuration

### §10.1 Gateway configuration overview

**Spec:** §10.1 (PDF) — configuration model for the UA-DDS gateway
(connections, address mappings, QoS).

**Repo:** cross-ref §10.2 + code in `crates/opcua-gateway/`.

**Tests:** —

**Status:** `n/a (informative)`

### §10.2 XML configuration

**Spec:** §10, p. 109-127 — XML configuration schema for gateway bridge defs
(UAtoDDS / DDStoUA connections).

**Repo:** `crates/opcua-gateway/src/xml.rs::parse_gateway_config`
(`roxmltree`-based, the same backend choice as `crates/xml/src/qos.rs` +
`crates/security-permissions/`). Data model: `GatewayConfig` with
`Vec<BridgeDef>`, each bridge with a domain ID + n `UaConnection` entries,
both directions (`ConnectionDirection::UaToDds` / `DdsToUa`), an optional
`browse_path` + `XmlNodeId` (numeric or string).

**Tests:** `xml::tests::parses_full_two_bridge_sample`,
`ua_to_dds_connection_yields_expected_fields`,
`dds_to_ua_connection_uses_string_node_id`,
`auxiliary_bridge_omits_optional_node_id_and_browse_path`,
`malformed_xml_yields_parse_error`,
`wrong_root_element_yields_unexpected_root_error`,
`missing_dds_topic_yields_missing_element_error`,
`invalid_numeric_node_id_yields_specific_error`,
`bridge_without_name_yields_missing_attribute_error`.

**Status:** done

---

## Audit status

15 done / 0 partial / 0 open / 8 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-opcua-gateway` — 133 lib tests green,
0 failed. Modules with tests: `address_space`, `conformance`,
`data_value`, `dds_to_ua::naming`, `dds_to_ua::node_spec`,
`dds_to_ua::walker`, `historical::adapter`, `historical::config`,
`historical::qos`, `node_id`, `service_sets`
(+ `attribute`/`method`/`query`/`view`), `subscription_mapping`
(+ `behavior`/`config`/`variant_dds`), `types`, `xml`.
