// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Output model — symbolic OPC-UA node specs.
//!
//! The walker emits, per DDS type, a list of [`NodeSpec`]
//! entries that the caller materializes into an OPC-UA AddressSpace.
//! NodeIds are referenced symbolically by browse names — the
//! caller (server-stack implementer) allocates the real NodeIds.

use alloc::string::String;
use alloc::vec::Vec;

/// OPC-UA NodeClass — the subset that §9.2 produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeClass {
    /// `DataType` node (Spec §8.3.1 Tab 8.3 NodeClass=64).
    DataType,
    /// `VariableType` node (16).
    VariableType,
    /// `Variable` node (2).
    Variable,
    /// `Object` node (1) — for map types (Spec Tab 9.40).
    Object,
    /// `ObjectType` node (8) — for map wrappers (Tab 9.40).
    ObjectType,
}

/// Symbolic type reference (BrowseName). The caller maps this to
/// a NodeId.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeRef(pub String);

impl TypeRef {
    /// Convenient constructor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// OPC-UA Reference-Kind aus §9.2 (Subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    /// `HasSubtype` — inheritance of DataTypes (Tab 9.15 etc.).
    HasSubtype,
    /// `HasComponent` — member of struct/VariableType (Tab 9.16).
    HasComponent,
    /// `HasOrderedComponent` — element slot of array-of-struct
    /// (Tab 9.27).
    HasOrderedComponent,
    /// `HasProperty` — e.g. EnumValues, OptionSetValues.
    HasProperty,
    /// `HasTypeDefinition` — Variable → VariableType (Tab 9.18).
    HasTypeDefinition,
    /// `HasEncoding` — DataType → encoding object (rare in §9.2,
    /// but in Tab 9.45 for structures).
    HasEncoding,
}

/// Spec Tab 8.4-8.18 typical ValueRank values.
///
/// Spec: `ValueRank = -1` for a scalar, `>= 1` for an N-dim array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRank(pub i32);

impl ValueRank {
    /// Skalar (ValueRank = -1).
    pub const SCALAR: Self = Self(-1);
}

/// Variable-specific content (Spec Tab 9.3/9.18/9.24/etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSpec {
    /// `DataType` — Type-Reference.
    pub data_type: TypeRef,
    /// `ValueRank`.
    pub value_rank: ValueRank,
    /// `ArrayDimensions` (for ValueRank >= 1).
    pub array_dimensions: Vec<u32>,
    /// `HasTypeDefinition` target (typically `BaseDataVariableType`).
    pub type_definition: TypeRef,
}

impl Default for VariableSpec {
    fn default() -> Self {
        Self {
            data_type: TypeRef::new("BaseDataType"),
            value_rank: ValueRank::SCALAR,
            array_dimensions: Vec::new(),
            type_definition: TypeRef::new("BaseDataVariableType"),
        }
    }
}

/// A single OPC-UA node spec emitted by the walker. A
/// `NodeSpec` describes **one** node + its outgoing references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSpec {
    /// `BrowseName` — symbolic.
    pub browse_name: String,
    /// NodeClass.
    pub node_class: NodeClass,
    /// `IsAbstract` (relevant for DataType/VariableType).
    pub is_abstract: bool,
    /// Subtype-of reference (Tab 9.15/9.20: a DataType inherits from a
    /// standard root such as `Structure`/`Union`/`Enumeration`).
    pub subtype_of: Option<TypeRef>,
    /// `Variable` data — only for `NodeClass == Variable`.
    pub variable: Option<VariableSpec>,
    /// Outgoing references (browse-name-based).
    pub references: Vec<OutgoingRef>,
}

/// An outgoing reference from a node — Spec Tab 9.27 etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingRef {
    /// Reference kind (`HasComponent`, `HasOrderedComponent`, ...).
    pub kind: ReferenceKind,
    /// Target BrowseName.
    pub target: TypeRef,
}

impl NodeSpec {
    /// Constructor for a DataType node (Spec Tab 9.15 style).
    #[must_use]
    pub fn data_type(browse_name: impl Into<String>, subtype_of: TypeRef) -> Self {
        Self {
            browse_name: browse_name.into(),
            node_class: NodeClass::DataType,
            is_abstract: false,
            subtype_of: Some(subtype_of),
            variable: None,
            references: Vec::new(),
        }
    }

    /// Constructor for a VariableType node (Spec Tab 9.16).
    #[must_use]
    pub fn variable_type(browse_name: impl Into<String>, data_type: TypeRef) -> Self {
        Self {
            browse_name: browse_name.into(),
            node_class: NodeClass::VariableType,
            is_abstract: false,
            subtype_of: Some(TypeRef::new("BaseDataVariableType")),
            variable: Some(VariableSpec {
                data_type,
                ..VariableSpec::default()
            }),
            references: Vec::new(),
        }
    }

    /// Constructor for a Variable node (Spec Tab 9.19/9.27).
    #[must_use]
    pub fn variable(browse_name: impl Into<String>, var: VariableSpec) -> Self {
        Self {
            browse_name: browse_name.into(),
            node_class: NodeClass::Variable,
            is_abstract: false,
            subtype_of: None,
            variable: Some(var),
            references: Vec::new(),
        }
    }

    /// Adds an outgoing reference.
    pub fn with_ref(mut self, kind: ReferenceKind, target: TypeRef) -> Self {
        self.references.push(OutgoingRef { kind, target });
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn data_type_constructor_carries_subtype() {
        let n = NodeSpec::data_type("FooDataType", TypeRef::new("Structure"));
        assert_eq!(n.node_class, NodeClass::DataType);
        assert_eq!(n.subtype_of, Some(TypeRef::new("Structure")));
    }

    #[test]
    fn variable_default_is_scalar_base_data_type() {
        let v = VariableSpec::default();
        assert_eq!(v.value_rank, ValueRank::SCALAR);
        assert!(v.array_dimensions.is_empty());
        assert_eq!(v.data_type.0, "BaseDataType");
    }

    #[test]
    fn with_ref_appends_reference() {
        let n = NodeSpec::data_type("FooDataType", TypeRef::new("Structure"))
            .with_ref(ReferenceKind::HasComponent, TypeRef::new("bar"));
        assert_eq!(n.references.len(), 1);
        assert_eq!(n.references[0].kind, ReferenceKind::HasComponent);
        assert_eq!(n.references[0].target.0, "bar");
    }
}
