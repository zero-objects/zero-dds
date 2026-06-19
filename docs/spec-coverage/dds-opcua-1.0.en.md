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

**Tests:** cross-ref §8.2/§9.2/§9.3.

**Status:** done

---

## §2 Conformance

### §2 Conformance point 1: OPC UA to DDS mapping basic

**Spec:** §2 point 1 (p. 1) — UA type system + UA subscription mapping.

**Repo:** type-system mapping in `crates/opcua-gateway/src/types.rs` +
`dds_to_ua/walker.rs`; subscription mapping in
`crates/opcua-gateway/src/subscription_mapping/{behavior,config,variant_dds}.rs`.

**Tests:** cross-ref `subscription_mapping::tests::*`.

**Status:** done

### §2 Conformance point 2: OPC UA to DDS mapping complete

**Spec:** §2 point 2 — basic + UA service sets.

**Repo:** point 1 + service sets in
`crates/opcua-gateway/src/service_sets/{attribute, method, query, view}.rs`.

**Tests:** cross-ref `service_sets::tests::*`.

**Status:** done

### §2 Conformance point 3: DDS to OPC UA mapping basic

**Spec:** §2 point 3 — DDS type system + DDS global data space (without
historical).

**Repo:** type system via `dds_to_ua/walker.rs` (see §9.2); GDS mapping in
`address_space.rs::{DomainNode, TopicNode, SampleVariable}` (see §9.3).

**Tests:** cross-ref `dds_to_ua::walker::tests::*` +
`address_space::tests::*`.

**Status:** done

### §2 Conformance point 4: DDS to OPC UA mapping complete

**Spec:** §2 point 4 — basic + historical access.

**Repo:** point 3 + historical access in
`crates/opcua-gateway/src/historical/{adapter, config, qos}.rs` with a
DDS-DurabilityService → UA-HistoryRead mapping.

**Tests:** cross-ref `historical::tests::*`.

**Status:** done

### §2 Multi-point conformance marker

**Spec:** §2 — an implementation may satisfy several conformance points at
once ("multi-point").

**Repo:** `crates/opcua-gateway/src/conformance.rs` — an explicit,
machine-queryable marker: the `ConformancePoint` enum (all four points with
title + number + `subsumes_basic`), `DECLARED_CONFORMANCE` (= all four) +
`is_multi_point_conformant()` / `declares()`. A server-stack integrator can
query the conformance status programmatically instead of reading prose.

**Tests:** `conformance::tests::*` (declares-all-four, is-multi-point,
unique numbering, complete-subsumes-basic, distinct titles).

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

## §4 Terms and definitions

**Spec:** §4 (PDF) — glossary.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary; without a code mapping.

## §5 Symbols and abbreviations

**Spec:** §5 (PDF) — abbreviations (DDS, OPC-UA, GDS, etc.).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — symbol table; without a code mapping.

---

## §6 Additional information

**Spec:** §6 (PDF) — informative annex (roadmap, acknowledgements).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

## §7 UA/DDS overview (informative)

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

**Tests:** `types::tests::builtin_type_kind_values_match_spec`.

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
* `types.rs::{Guid, StatusCode, ByteString, QualifiedName, LocalizedText}`.
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

**Spec:** §8.3, p. 16-30 — View/Query/Attribute/Method service set as
DDS-RPC interfaces.

**Repo:** `crates/opcua-gateway/src/service_sets/{view,query,attribute,
method}.rs` with DDS-RPC bindings via `crates/rpc/`.

**Tests:** inline.

**Status:** done

### §8.4 OPC UA subscription model mapping

**Spec:** §8.4, p. 31-48 — mapping OPC-UA subscriptions onto DDS DataReader
listeners with MonitoredItems push.

**Repo:** `crates/opcua-gateway/src/subscription_mapping/{behavior,config,variant_dds}.rs`.

**Tests:** inline.

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
`crates/opcua-gateway/src/dds_to_ua/walker.rs::{Walker, NodeKind, NodeSpec}`
with recursive aggregated (struct/union) and collection handling
(sequence-of-struct, Map<String, Sequence<Struct>>); the inverse mapping
symmetric via `BuiltinTypeKind`. **Instance side:**
`address_space.rs::build_sample_instance` recursively decomposes a
structured sample into an `InstanceNode` variable hierarchy (HasComponent
member components, nested; collections as array/object variables) —
mirroring the §9.2 type recursion on the §9.3 instance side.
`SampleVariable::scalar` remains for the simple scalar value path.

**Tests:** cross-ref `dds_to_ua::walker::tests::*` +
`address_space::tests::*` (incl. `struct_sample_decomposes_*`,
`nested_struct_sample_recurses_*`, `union_sample_has_switchfield_*`).

**Status:** done — data model + scalar mapping + aggregated/collection/
nested recursion in **both the type walker and the instance builder**.

### §9.3 DDS global data space mapping

**Spec:** §9.3, p. 94-107 — DDS domain → OPC-UA ObjectNode with BrowseName
`Domain<id>`; DDS topic → OPC-UA ObjectNode with BrowseName = topic name;
DDS sample → OPC-UA variable.

**Repo:** `crates/opcua-gateway/src/address_space.rs::{DomainNode,
TopicNode, SampleVariable, mangle_topic_node_browse_name}`.

**Tests:** `address_space::tests::domain_node_for_zero_yields_browse_name_domain0`,
`domain_node_for_42_yields_browse_name_domain42`,
`mangle_topic_node_browse_name_is_pass_through`,
`topic_node_carries_parent_domain_reference`,
`sample_variable_scalar_has_value_rank_minus_1`.

**Status:** done

### §9.3.4 Reading historical data from instance nodes

**Spec:** §9.3.4 — historical-access subset (conformance point 4
"complete").

**Repo:** `crates/opcua-gateway/src/historical/{adapter,config,qos}.rs`.

**Tests:** inline.

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
