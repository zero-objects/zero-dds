// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS → OPC UA Type Mapping — Spec §9.2.
//!
//! Bildet beliebige DDS-XTypes-Typen rekursiv auf OPC-UA-Nodes
//! (DataType + VariableType + Variable + References) ab. Die Mapping-
//! Regeln aus §9.2.1-§9.2.8 sind hier vollstaendig:
//!
//! * `§9.2.1 Primitive Types` — Tab 9.1/9.2 (Boolean → Boolean, Int32
//!   → Int32, ...).
//! * `§9.2.2 String Types` — String8/String16 als String-Variants.
//! * `§9.2.3 Enumerated Types` — `<EnumName>DataType` als Enumeration
//!   mit EnumValues-Property.
//! * `§9.2.3 Bitmask Types` (Tab 9.11-9.14) — `<BitmaskName>DataType`
//!   als OptionSet + Gap-Auffuellung mit `UndefinedPosition_<N>`.
//! * `§9.2.4 Aggregated Types` (Struct + Union):
//!   - Struct → DataType + VariableType + HasComponent References.
//!   - Union → UnionDataType (subtype Union) mit consecutive switch
//!     values 1..N.
//! * `§9.2.5 Collection Types` (Array + Sequence + Map):
//!   - Array of Primitive/String → Variable mit ValueRank=N≥1.
//!   - Array of Enum/Bitmask → wie Skalar, aber ValueRank=N.
//!   - Array of Struct/Union → ValueRank≥1 + HasOrderedComponent
//!     References pro Element zu `<TypeName>_<index>`-Variables.
//!   - Sequence — wie Array, aber unbeschraenkt (max bound im
//!     ArrayDimensions-Wert reflektiert).
//!   - Map — Tab 9.40/9.41 als Object mit MapEntry-Variables.
//! * `§9.2.6 Nested Types` — Rekursion: Struct-im-Struct, Sequence-im-
//!   Map, etc. greifen unveraendert auf den gleichen Walker zu.
//! * `§9.2.7 Alias Types` — durchsichtig (Walker resolved den Alias).
//! * `§9.2.8 Keyed Types` — Annotations werden an die DataType-Nodes
//!   propagiert (Property `IsKey: Boolean[]`).
//!
//! # Output-Modell
//!
//! [`NodeSpec`] ist der symbolische Output: BrowseName, NodeClass,
//! DataType-Reference (per Symbol-Name, kein NodeId), HasComponent /
//! HasSubtype / HasOrderedComponent References. Der Caller materialisiert
//! das in OPC-UA-Server-AddressSpace-API-Calls oder XML-NodeSet-
//! Files. Dieses Modul liefert die Spec-konforme Schema-Information,
//! nicht die Wire-Encoding-Schicht.

pub mod naming;
pub mod node_spec;
pub mod walker;

pub use naming::{
    array_element_browse_name, data_type_name, undefined_bitfield_position, variable_type_name,
};
pub use node_spec::{NodeClass as UaNodeClass, NodeSpec, ReferenceKind, TypeRef, VariableSpec};
pub use walker::{
    BitmaskDef, DdsType, EnumDef, MemberDef, MemberKind, StructDef, UnionCase, UnionDef, WalkError,
    WalkOutput, walk_dds_type,
};
