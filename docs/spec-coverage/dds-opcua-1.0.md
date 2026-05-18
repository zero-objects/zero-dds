# DDS-OPC UA Gateway 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/dds-opcua-1.0.pdf` (OMG formal/2020-01-01).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** DDS-OPC-UA-Gateway bridge zwischen DDS-Topics und
OPC-UA-AddressSpace (IEC 62541). ZeroDDS implementiert die
**Type-System-Mapping**-Layer (§8.2 + §9.2) sowie das
**AddressSpace-Mapping** (§9.3) als pure-Rust no_std+alloc Library
in `crates/opcua-gateway/`. Subscription-Behavior + OPC-UA-Binary-
Wire-Encoding sind Caller-Layer (typisch externer OPC-UA-Stack).

Implementation: `crates/opcua-gateway/` (4 Module, 33 Tests gruen).

---

## §1 Scope

### §1 Bidirectional Bridge OPC UA ↔ DDS

**Spec:** §1, S. 1 (PDF) — "This specification overcomes this
situation by defining a standard, vendor-independent, configurable
gateway that enables interoperability and information exchange between
systems that use DDS and systems that use OPC UA."

**Repo:** `crates/opcua-gateway/src/lib.rs` Crate-Doc.

**Tests:** Cross-Ref §8.2/§9.2/§9.3.

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

**Tests:** Cross-Ref `subscription_mapping::tests::*`.

**Status:** done

### §2 Conformance Point 2: OPC UA to DDS Mapping Complete

**Spec:** §2 Punkt 2 — Basic + UA Service-Sets.

**Repo:** Punkt 1 + Service-Sets in
`crates/opcua-gateway/src/service_sets/{attribute, method, query,
view}.rs`.

**Tests:** Cross-Ref `service_sets::tests::*`.

**Status:** done

### §2 Conformance Point 3: DDS to OPC UA Mapping Basic

**Spec:** §2 Punkt 3 — DDS Type-System + DDS Global Data Space
(without Historical).

**Repo:** Type-System via `dds_to_ua/walker.rs` (siehe §9.2);
GDS-Mapping in `address_space.rs::{DomainNode, TopicNode,
SampleVariable}` (siehe §9.3).

**Tests:** Cross-Ref `dds_to_ua::walker::tests::*` +
`address_space::tests::*`.

**Status:** done

### §2 Conformance Point 4: DDS to OPC UA Mapping Complete

**Spec:** §2 Punkt 4 — Basic + Historical Access.

**Repo:** Punkt 3 + Historical-Access in
`crates/opcua-gateway/src/historical/{adapter, config, qos}.rs`
mit DDS-DurabilityService → UA-HistoryRead-Mapping.

**Tests:** Cross-Ref `historical::tests::*`.

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

**Status:** `n/a (informative)` — Externe normative Referenz-Liste; OPCUA-/DDS-Specs werden in den Konsumenten-Items §8/§9 operativ erfuellt.

---

## §4 Terms and Definitions

**Spec:** §4 (PDF) — Glossar.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar; ohne Code-Mapping.

## §5 Symbols and Abbreviations

**Spec:** §5 (PDF) — Abkürzungen (DDS, OPC-UA, GDS, etc.).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Symbol-Tabelle; ohne Code-Mapping.

---

## §6 Additional Information

**Spec:** §6 (PDF) — informativer Anhang (Roadmap, Acknowledgements).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

## §7 UA/DDS Overview (Informative)

**Spec:** §7 (PDF) — non-normative Overview-Material zu UA + DDS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert §7 explizit als
non-normative Overview.

---

## §8 OPC UA to DDS Bridge

### §8.1 OPC UA to DDS Bridge Overview

**Spec:** §8.1 (PDF) — Bridge-Architektur-Überblick (informativ).

**Repo:** Cross-Ref §8.2-§8.4.

**Tests:** —

**Status:** `n/a (informative)`

### §8.2 OPC UA Type System Mapping

**Spec:** §8.2, S. 11-12 (PDF) — `BuiltinTypeKind` Enum mit allen 25
OPC-UA-Built-in-Type-IDs (Boolean=1, ..., DiagnosticInfo=25).

**Repo:** `crates/opcua-gateway/src/types.rs::BuiltinTypeKind` mit
allen 25 Werten + `value()`-Methode.

**Tests:** `types::tests::builtin_type_kind_values_match_spec`.

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
* `types.rs::{Guid, StatusCode, ByteString, QualifiedName,
  LocalizedText}`.
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
exposed; das mutable @optional-Felder-Modell waere analog zu
`LocalizedText` ergaenzbar.

### §8.3 OPC UA Service Sets Mapping

**Spec:** §8.3, S. 16-30 — View/Query/Attribute/Method-Service-Set als
DDS-RPC-Interfaces.

**Repo:** `crates/opcua-gateway/src/service_sets/{view,query,attribute,
method}.rs` mit DDS-RPC-Bindings über `crates/rpc/`.

**Tests:** Inline.

**Status:** done

### §8.4 OPC UA Subscription Model Mapping

**Spec:** §8.4, S. 31-48 — Mapping von OPC-UA-Subscriptions auf
DDS-DataReader-Listener mit MonitoredItems-Push.

**Repo:** `crates/opcua-gateway/src/subscription_mapping/{behavior,
config,variant_dds}.rs`.

**Tests:** Inline.

**Status:** done

---

## §9 DDS to OPC UA Bridge

### §9.1 DDS to OPC UA Bridge Overview

**Spec:** §9.1 (PDF) — Bridge-Architektur DDS→UA (informativ).

**Repo:** Cross-Ref §9.2-§9.3.

**Tests:** —

**Status:** `n/a (informative)`

### §9.2 DDS Type System Mapping

**Spec:** §9.2, S. 49-93 — DDS-Primitive/String/Enum/Aggregated/
Collection/Nested/Alias/Keyed-Types → OPC-UA-Variables.

**Repo:** Vollstaendiges DDS→UA-Type-Walking in
`crates/opcua-gateway/src/dds_to_ua/walker.rs::{Walker, NodeKind,
NodeSpec}` mit rekursiver Aggregated- (Struct/Union) und Collection-
Behandlung (Sequence-of-Struct, Map<String, Sequence<Struct>>);
inverses Mapping symmetrisch via `BuiltinTypeKind`.
AddressSpace-Builder in `address_space.rs::SampleVariable::scalar`
fuer den Wert-Pfad.

**Tests:** Cross-Ref `dds_to_ua::walker::tests::*` +
`address_space::tests::*`.

**Status:** done — Datenmodell + Scalar-Mapping + Aggregated/
Collection-Recursion via Walker abgedeckt.

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

**Spec:** §9.3.4 — Historical-Access-Subset (Conformance-Point 4
"Complete").

**Repo:** `crates/opcua-gateway/src/historical/{adapter,config,
qos}.rs`.

**Tests:** Inline.

**Status:** done

---

## §10 OPC UA/DDS Gateway Configuration

### §10.1 Gateway Configuration Overview

**Spec:** §10.1 (PDF) — Configuration-Modell für UA-DDS-Gateway
(Connections, Address-Mappings, QoS).

**Repo:** Cross-Ref §10.2 + Code in `crates/opcua-gateway/`.

**Tests:** —

**Status:** `n/a (informative)`

### §10.2 XML Configuration

**Spec:** §10, S. 109-127 — XML-Configuration-Schema fuer
Gateway-Bridge-Defs (UAtoDDS / DDStoUA Connections).

**Repo:** `crates/opcua-gateway/src/xml.rs::parse_gateway_config`
(`roxmltree`-basiert, gleiche Backend-Wahl wie `crates/xml/src/qos.rs`
+ `crates/security-permissions/`). Datenmodell: `GatewayConfig` mit
`Vec<BridgeDef>`, jede Bridge mit Domain-Id + n
`UaConnection`-Eintraegen, beide Richtungen
(`ConnectionDirection::UaToDds` / `DdsToUa`), optionalem
`browse_path` + `XmlNodeId` (numeric oder string).

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

## Audit-Status

13 done / 0 partial / 0 open / 8 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-opcua-gateway` — 119 lib-Tests grün,
0 failed. Module mit Tests: `address_space`, `data_value`,
`dds_to_ua::naming`, `dds_to_ua::node_spec`, `dds_to_ua::walker`,
`historical::adapter`, `historical::config`, `historical::qos`,
`node_id`, `service_sets` (+ `attribute`/`method`/`query`/`view`),
`subscription_mapping` (+ `behavior`/`config`/`variant_dds`),
`types`, `xml`.

Offene/partielle Punkte: siehe `dds-opcua-1.0.open.md`. §8.3 Service-
Sets, §8.4 Subscription, §9.3.4 Historical sind alle in den
opcua-gateway-Submodulen implementiert; verbleiben §2 Conformance-
Multi-Punkt-Marker und §9.2 Aggregated/Collection-Recursion (Caller-
Layer).
