// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Output-Modell — symbolische OPC-UA Node-Specs.
//!
//! Der Walker emittiert pro DDS-Type eine Liste von [`NodeSpec`]-
//! Eintraegen, die der Caller in einen OPC-UA-AddressSpace materialisiert.
//! NodeIds werden symbolisch ueber Browse-Namen referenziert — der
//! Caller (Server-Stack-Implementer) allokiert die echten NodeIds.

use alloc::string::String;
use alloc::vec::Vec;

/// OPC-UA NodeClass — Subset, das §9.2 erzeugt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeClass {
    /// `DataType`-Node (Spec §8.3.1 Tab 8.3 NodeClass=64).
    DataType,
    /// `VariableType`-Node (16).
    VariableType,
    /// `Variable`-Node (2).
    Variable,
    /// `Object`-Node (1) — fuer Map-Typen (Spec Tab 9.40).
    Object,
    /// `ObjectType`-Node (8) — fuer Map-Wrapper (Tab 9.40).
    ObjectType,
}

/// Symbolische Type-Reference (BrowseName). Der Caller mappt das auf
/// einen NodeId.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeRef(pub String);

impl TypeRef {
    /// Bequemer Konstruktor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// OPC-UA Reference-Kind aus §9.2 (Subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    /// `HasSubtype` — Vererbung der DataTypes (Tab 9.15 etc.).
    HasSubtype,
    /// `HasComponent` — Member von Struct/VariableType (Tab 9.16).
    HasComponent,
    /// `HasOrderedComponent` — Element-Slot von Array-of-Struct
    /// (Tab 9.27).
    HasOrderedComponent,
    /// `HasProperty` — z.B. EnumValues, OptionSetValues.
    HasProperty,
    /// `HasTypeDefinition` — Variable → VariableType (Tab 9.18).
    HasTypeDefinition,
    /// `HasEncoding` — DataType → Encoding-Object (selten in §9.2,
    /// aber in Tab 9.45 fuer Structures).
    HasEncoding,
}

/// Spec Tab 8.4-8.18 typische ValueRank-Werte.
///
/// Spec: `ValueRank = -1` fuer Scalar, `>= 1` fuer N-Dim-Array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRank(pub i32);

impl ValueRank {
    /// Skalar (ValueRank = -1).
    pub const SCALAR: Self = Self(-1);
}

/// Variable-spezifischer Inhalt (Spec Tab 9.3/9.18/9.24/etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSpec {
    /// `DataType` — Type-Reference.
    pub data_type: TypeRef,
    /// `ValueRank`.
    pub value_rank: ValueRank,
    /// `ArrayDimensions` (fuer ValueRank >= 1).
    pub array_dimensions: Vec<u32>,
    /// `HasTypeDefinition` Ziel (typisch `BaseDataVariableType`).
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

/// Eine einzelne OPC-UA-Node-Spec, die der Walker emittiert. Eine
/// `NodeSpec` beschreibt **eine** Node + ausgehende References.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSpec {
    /// `BrowseName` — symbolisch.
    pub browse_name: String,
    /// NodeClass.
    pub node_class: NodeClass,
    /// `IsAbstract` (relevant fuer DataType/VariableType).
    pub is_abstract: bool,
    /// Subtype-Of-Verweis (Tab 9.15/9.20: DataType erbt von einer
    /// Standard-Wurzel wie `Structure`/`Union`/`Enumeration`).
    pub subtype_of: Option<TypeRef>,
    /// `Variable`-Daten — nur bei `NodeClass == Variable`.
    pub variable: Option<VariableSpec>,
    /// Ausgehende References (Browse-Name-basiert).
    pub references: Vec<OutgoingRef>,
}

/// Eine ausgehende Reference von einer Node — Spec Tab 9.27 etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingRef {
    /// Reference-Kind (`HasComponent`, `HasOrderedComponent`, ...).
    pub kind: ReferenceKind,
    /// Ziel-BrowseName.
    pub target: TypeRef,
}

impl NodeSpec {
    /// Konstruktor fuer eine DataType-Node (Spec Tab 9.15-Stil).
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

    /// Konstruktor fuer eine VariableType-Node (Spec Tab 9.16).
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

    /// Konstruktor fuer eine Variable-Node (Spec Tab 9.19/9.27).
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

    /// Fuegt eine ausgehende Reference hinzu.
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
