// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC-UA Built-in Types + Primitive-Type-Mapping zu DDS — Spec §8.2.

use alloc::string::String;
use alloc::vec::Vec;

/// `BuiltinTypeKind` (Spec §8.2 enum, S. 11-12) — alle 25 OPC-UA
/// Built-in-Type-IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinTypeKind {
    /// `1` Boolean.
    Boolean,
    /// `2` SByte (int8).
    SByte,
    /// `3` Byte (uint8).
    Byte,
    /// `4` Int16.
    Int16,
    /// `5` UInt16.
    UInt16,
    /// `6` Int32.
    Int32,
    /// `7` UInt32.
    UInt32,
    /// `8` Int64.
    Int64,
    /// `9` UInt64.
    UInt64,
    /// `10` Float.
    Float,
    /// `11` Double.
    Double,
    /// `12` String (UTF-8).
    String,
    /// `13` DateTime (int64 ticks).
    DateTime,
    /// `14` Guid.
    Guid,
    /// `15` ByteString.
    ByteString,
    /// `16` XmlElement.
    XmlElement,
    /// `17` NodeId.
    NodeId,
    /// `18` ExpandedNodeId.
    ExpandedNodeId,
    /// `19` StatusCode.
    StatusCode,
    /// `20` QualifiedName.
    QualifiedName,
    /// `21` LocalizedText.
    LocalizedText,
    /// `22` ExtensionObject.
    ExtensionObject,
    /// `23` DataValue.
    DataValue,
    /// `24` Variant.
    Variant,
    /// `25` DiagnosticInfo.
    DiagnosticInfo,
}

impl BuiltinTypeKind {
    /// `@value(N)` aus Spec — kanonischer Wert.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Boolean => 1,
            Self::SByte => 2,
            Self::Byte => 3,
            Self::Int16 => 4,
            Self::UInt16 => 5,
            Self::Int32 => 6,
            Self::UInt32 => 7,
            Self::Int64 => 8,
            Self::UInt64 => 9,
            Self::Float => 10,
            Self::Double => 11,
            Self::String => 12,
            Self::DateTime => 13,
            Self::Guid => 14,
            Self::ByteString => 15,
            Self::XmlElement => 16,
            Self::NodeId => 17,
            Self::ExpandedNodeId => 18,
            Self::StatusCode => 19,
            Self::QualifiedName => 20,
            Self::LocalizedText => 21,
            Self::ExtensionObject => 22,
            Self::DataValue => 23,
            Self::Variant => 24,
            Self::DiagnosticInfo => 25,
        }
    }

    /// Liefert die DDS-IDL-Equivalent-Type-Bezeichnung als String
    /// gemaess Spec §8.2.1 Tab 8.1 (Primitive-Mapping).
    ///
    /// Liefert `None` fuer Composite-Types — diese haben strukturierte
    /// IDL-Equivalents (siehe Tab 8.2), die ueber spezielle Module
    /// (`Guid`, `NodeId`, etc.) abgebildet werden.
    #[must_use]
    pub const fn dds_idl_primitive(self) -> Option<&'static str> {
        match self {
            Self::Boolean => Some("boolean"),
            Self::SByte => Some("int8"),
            Self::Byte => Some("uint8"),
            Self::Int16 => Some("int16"),
            Self::UInt16 => Some("uint16"),
            Self::Int32 => Some("int32"),
            Self::UInt32 => Some("uint32"),
            Self::Int64 => Some("int64"),
            Self::UInt64 => Some("uint64"),
            Self::Float => Some("float"),
            Self::Double => Some("double"),
            Self::String => Some("string"),
            _ => None,
        }
    }

    /// Liefert das DDS-XTYPES-Equivalent-Token gemaess Tab 8.1.
    /// Composite-Types liefern den Type-Namen (z.B. `"Guid"`).
    #[must_use]
    pub const fn dds_xtypes_name(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::SByte | Self::Byte => "Byte",
            Self::Int16 => "Int16",
            Self::UInt16 => "UInt16",
            Self::Int32 => "Int32",
            Self::UInt32 => "UInt32",
            Self::Int64 => "Int64",
            Self::UInt64 => "UInt64",
            Self::Float => "Float32",
            Self::Double => "Float64",
            Self::String => "String8",
            Self::DateTime => "DateTime",
            Self::Guid => "Guid",
            Self::ByteString => "ByteString",
            Self::XmlElement => "XmlElement",
            Self::NodeId => "NodeId",
            Self::ExpandedNodeId => "ExpandedNodeId",
            Self::StatusCode => "StatusCode",
            Self::QualifiedName => "QualifiedName",
            Self::LocalizedText => "LocalizedText",
            Self::ExtensionObject => "ExtensionObject",
            Self::DataValue => "DataValue",
            Self::Variant => "Variant",
            Self::DiagnosticInfo => "DiagnosticInfo",
        }
    }
}

/// Spec §8.2.1 Tab 8.1 — direktes Primitive-Mapping als ergonomische
/// Funktion. Liefert `None` fuer non-Primitive-Types.
#[must_use]
pub const fn map_primitive_to_dds(t: BuiltinTypeKind) -> Option<&'static str> {
    t.dds_idl_primitive()
}

/// Spec §8.3.1 Tab 8.3 — `NodeClass` Enum (8 Werte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeClass {
    /// `1` ObjectNode.
    Object,
    /// `2` VariableNode.
    Variable,
    /// `4` MethodNode.
    Method,
    /// `8` ObjectTypeNode.
    ObjectType,
    /// `16` VariableTypeNode.
    VariableType,
    /// `32` ReferenceTypeNode.
    ReferenceType,
    /// `64` DataTypeNode.
    DataType,
    /// `128` ViewNode.
    View,
}

impl NodeClass {
    /// Spec @value(N) — bit-flag-style Werte.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Object => 1,
            Self::Variable => 2,
            Self::Method => 4,
            Self::ObjectType => 8,
            Self::VariableType => 16,
            Self::ReferenceType => 32,
            Self::DataType => 64,
            Self::View => 128,
        }
    }
}

/// Spec §8.2.2 — `Guid` als Struct (data1 + data2 + data3 + data4[8])
/// gemaess RFC 4122-Layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Guid {
    /// `data1`: uint32.
    pub data1: u32,
    /// `data2`: uint16.
    pub data2: u16,
    /// `data3`: uint16.
    pub data3: u16,
    /// `data4`: octet[8].
    pub data4: [u8; 8],
}

impl Guid {
    /// Konstruiert einen Guid aus 16 Bytes (RFC 4122 Big-Endian-Layout).
    #[must_use]
    pub fn from_bytes(b: [u8; 16]) -> Self {
        Self {
            data1: u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            data2: u16::from_be_bytes([b[4], b[5]]),
            data3: u16::from_be_bytes([b[6], b[7]]),
            data4: [b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]],
        }
    }

    /// Liefert die 16-Byte-Wire-Form (RFC 4122 BE-Layout).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.data1.to_be_bytes());
        out[4..6].copy_from_slice(&self.data2.to_be_bytes());
        out[6..8].copy_from_slice(&self.data3.to_be_bytes());
        out[8..16].copy_from_slice(&self.data4);
        out
    }
}

/// `StatusCode` (Spec §8.2.2) — `uint32` Bitfeld.
pub type StatusCode = u32;

/// Spec §8.2.2 — `ByteString` = `sequence<octet>`.
pub type ByteString = Vec<u8>;

/// Spec §8.2.2 — `QualifiedName` mit namespace_index + name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    /// Namespace-Index.
    pub namespace_index: u16,
    /// Name (max. 512 Zeichen per Spec).
    pub name: String,
}

/// Spec §8.2.2 (LocalizedText, mutable + optional Felder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedText {
    /// `@id(1) @optional string locale`.
    pub locale: Option<String>,
    /// `@id(2) @optional string text`.
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_type_kind_values_match_spec() {
        // Spec §8.2 Enum-Werte 1..=25.
        assert_eq!(BuiltinTypeKind::Boolean.value(), 1);
        assert_eq!(BuiltinTypeKind::SByte.value(), 2);
        assert_eq!(BuiltinTypeKind::Byte.value(), 3);
        assert_eq!(BuiltinTypeKind::Int32.value(), 6);
        assert_eq!(BuiltinTypeKind::Double.value(), 11);
        assert_eq!(BuiltinTypeKind::String.value(), 12);
        assert_eq!(BuiltinTypeKind::NodeId.value(), 17);
        assert_eq!(BuiltinTypeKind::Variant.value(), 24);
        assert_eq!(BuiltinTypeKind::DiagnosticInfo.value(), 25);
    }

    #[test]
    fn primitive_mapping_matches_spec_table_8_1() {
        // Spec Tab 8.1.
        assert_eq!(
            BuiltinTypeKind::Boolean.dds_idl_primitive(),
            Some("boolean")
        );
        assert_eq!(BuiltinTypeKind::SByte.dds_idl_primitive(), Some("int8"));
        assert_eq!(BuiltinTypeKind::Byte.dds_idl_primitive(), Some("uint8"));
        assert_eq!(BuiltinTypeKind::Int16.dds_idl_primitive(), Some("int16"));
        assert_eq!(BuiltinTypeKind::UInt16.dds_idl_primitive(), Some("uint16"));
        assert_eq!(BuiltinTypeKind::Int32.dds_idl_primitive(), Some("int32"));
        assert_eq!(BuiltinTypeKind::UInt32.dds_idl_primitive(), Some("uint32"));
        assert_eq!(BuiltinTypeKind::Int64.dds_idl_primitive(), Some("int64"));
        assert_eq!(BuiltinTypeKind::UInt64.dds_idl_primitive(), Some("uint64"));
        assert_eq!(BuiltinTypeKind::Float.dds_idl_primitive(), Some("float"));
        assert_eq!(BuiltinTypeKind::Double.dds_idl_primitive(), Some("double"));
        assert_eq!(BuiltinTypeKind::String.dds_idl_primitive(), Some("string"));
    }

    #[test]
    fn composite_types_have_no_primitive_mapping() {
        // Spec §8.2.2 — Composite gehen ueber spezielle Mappings.
        assert_eq!(BuiltinTypeKind::Guid.dds_idl_primitive(), None);
        assert_eq!(BuiltinTypeKind::NodeId.dds_idl_primitive(), None);
        assert_eq!(BuiltinTypeKind::Variant.dds_idl_primitive(), None);
        assert_eq!(BuiltinTypeKind::DataValue.dds_idl_primitive(), None);
    }

    #[test]
    fn xtypes_names_match_spec_table_8_1() {
        // Spec Tab 8.1 DDS-Type-Spalte.
        assert_eq!(BuiltinTypeKind::Boolean.dds_xtypes_name(), "Boolean");
        assert_eq!(BuiltinTypeKind::SByte.dds_xtypes_name(), "Byte");
        assert_eq!(BuiltinTypeKind::Byte.dds_xtypes_name(), "Byte");
        assert_eq!(BuiltinTypeKind::Int32.dds_xtypes_name(), "Int32");
        assert_eq!(BuiltinTypeKind::Float.dds_xtypes_name(), "Float32");
        assert_eq!(BuiltinTypeKind::Double.dds_xtypes_name(), "Float64");
        assert_eq!(BuiltinTypeKind::String.dds_xtypes_name(), "String8");
    }

    #[test]
    fn sbyte_and_byte_both_map_to_dds_byte_in_xtypes() {
        // Spec §8.2.1 Footnote: "DDS-XTYPES does not define an 8-bit
        // signed integer; therefore, they are both mapped to DDS Bytes."
        assert_eq!(BuiltinTypeKind::SByte.dds_xtypes_name(), "Byte");
        assert_eq!(BuiltinTypeKind::Byte.dds_xtypes_name(), "Byte");
        // Aber im IDL-Equivalent unterschiedlich.
        assert_eq!(BuiltinTypeKind::SByte.dds_idl_primitive(), Some("int8"));
        assert_eq!(BuiltinTypeKind::Byte.dds_idl_primitive(), Some("uint8"));
    }

    #[test]
    fn map_primitive_to_dds_helper_yields_token() {
        assert_eq!(map_primitive_to_dds(BuiltinTypeKind::Float), Some("float"));
        assert_eq!(map_primitive_to_dds(BuiltinTypeKind::NodeId), None);
    }

    #[test]
    fn node_class_values_match_bit_flags() {
        // Spec Tab 8.3 — Bit-Flag-Werte.
        assert_eq!(NodeClass::Object.value(), 1);
        assert_eq!(NodeClass::Variable.value(), 2);
        assert_eq!(NodeClass::Method.value(), 4);
        assert_eq!(NodeClass::ObjectType.value(), 8);
        assert_eq!(NodeClass::VariableType.value(), 16);
        assert_eq!(NodeClass::ReferenceType.value(), 32);
        assert_eq!(NodeClass::DataType.value(), 64);
        assert_eq!(NodeClass::View.value(), 128);
    }

    #[test]
    fn guid_round_trips_via_bytes() {
        // RFC 4122 / Spec §8.2.2.
        let bytes = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x12, 0x34, 0x56, 0x78, 1, 2, 3, 4, 5, 6, 7, 8,
        ];
        let g = Guid::from_bytes(bytes);
        assert_eq!(g.data1, 0xDEAD_BEEF);
        assert_eq!(g.data2, 0x1234);
        assert_eq!(g.data3, 0x5678);
        assert_eq!(g.data4, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(g.to_bytes(), bytes);
    }

    #[test]
    fn qualified_name_carries_namespace_and_name() {
        // Spec Tab 8.2.
        let q = QualifiedName {
            namespace_index: 2,
            name: String::from("Temperature"),
        };
        assert_eq!(q.namespace_index, 2);
        assert_eq!(q.name, "Temperature");
    }

    #[test]
    fn localized_text_supports_optional_fields() {
        // Spec Tab 8.2 — @id(1)/(2) @optional.
        let lt = LocalizedText {
            locale: Some(String::from("de-DE")),
            text: Some(String::from("Sensor")),
        };
        assert!(lt.locale.is_some());
        assert!(lt.text.is_some());

        let empty = LocalizedText {
            locale: None,
            text: None,
        };
        assert!(empty.locale.is_none());
        assert!(empty.text.is_none());
    }
}
