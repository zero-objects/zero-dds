// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS → OPC UA Type Mapping — Spec §9.2.
//!
//! Maps arbitrary DDS-XTypes types recursively onto OPC-UA nodes
//! (DataType + VariableType + Variable + References). The mapping
//! rules from §9.2.1-§9.2.8 are complete here:
//!
//! * `§9.2.1 Primitive Types` — Tab 9.1/9.2 (Boolean → Boolean, Int32
//!   → Int32, ...).
//! * `§9.2.2 String Types` — String8/String16 as string variants.
//! * `§9.2.3 Enumerated Types` — `<EnumName>DataType` as an enumeration
//!   with an EnumValues property.
//! * `§9.2.3 Bitmask Types` (Tab 9.11-9.14) — `<BitmaskName>DataType`
//!   as an OptionSet + gap filling with `UndefinedPosition_<N>`.
//! * `§9.2.4 Aggregated Types` (struct + union):
//!   - Struct → DataType + VariableType + HasComponent references.
//!   - Union → UnionDataType (subtype Union) with consecutive switch
//!     values 1..N.
//! * `§9.2.5 Collection Types` (array + sequence + map):
//!   - Array of primitive/string → Variable with ValueRank=N≥1.
//!   - Array of enum/bitmask → like a scalar, but ValueRank=N.
//!   - Array of struct/union → ValueRank≥1 + HasOrderedComponent
//!     references per element to `<TypeName>_<index>` variables.
//!   - Sequence — like an array, but unbounded (max bound reflected
//!     in the ArrayDimensions value).
//!   - Map — Tab 9.40/9.41 as an Object with MapEntry variables.
//! * `§9.2.6 Nested Types` — recursion: struct-in-struct, sequence-in-
//!   map, etc. use the same walker unchanged.
//! * `§9.2.7 Alias Types` — transparent (the walker resolves the alias).
//! * `§9.2.8 Keyed Types` — annotations are propagated to the DataType
//!   nodes (property `IsKey: Boolean[]`).
//!
//! # Output model
//!
//! [`NodeSpec`] is the symbolic output: BrowseName, NodeClass,
//! DataType reference (by symbol name, no NodeId), HasComponent /
//! HasSubtype / HasOrderedComponent references. The caller materializes
//! this into OPC-UA server AddressSpace API calls or XML NodeSet
//! files. This module provides the spec-conformant schema information,
//! not the wire-encoding layer.

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
