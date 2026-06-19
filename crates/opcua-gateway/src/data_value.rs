// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Variant` + `DataValue` + `ExtensionObject` — Spec §8.2.2 Tab 8.2.

use alloc::string::String;
use alloc::vec::Vec;

use crate::node_id::NodeId;
use crate::types::{BuiltinTypeKind, ByteString, Guid, LocalizedText, QualifiedName, StatusCode};

/// Spec §8.2.2 — `BodyEncoding` enum (NONE/BYTESTRING/XMLELEMENT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyEncoding {
    /// `NONE_BODY_ENCODING` (Spec @value(0)).
    None,
    /// `BYTESTRING_BODY_ENCODING` (Spec @value(1)).
    ByteString,
    /// `XMLELEMENT_BODY_ENCODING` (Spec @value(2)).
    XmlElement,
}

/// Spec §8.2.2 — `ExtensionObject` with type_id + body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionObject {
    /// Spec — `NodeId type_id`.
    pub type_id: NodeId,
    /// Body encoding variant + bytes.
    pub body: ExtensionObjectBody,
}

/// Spec §8.2.2 — `ExtensionObjectBody` Union switch (BodyEncoding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionObjectBody {
    /// `NONE_BODY_ENCODING` Case — single octet (placeholder).
    None,
    /// `BYTESTRING_BODY_ENCODING` Case.
    ByteString(ByteString),
    /// `XMLELEMENT_BODY_ENCODING` Case (UTF-8 XML).
    XmlElement(String),
}

/// Spec §8.2.2 — `Variant` with array_dimensions + value sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Spec — `sequence<uint32> array_dimensions`. Empty = scalar,
    /// length=1 = 1D-array, length>1 = multi-dim.
    pub array_dimensions: Vec<u32>,
    /// Spec — `sequence<VariantValue> value`.
    pub value: Vec<VariantValue>,
}

impl Variant {
    /// Constructor for a scalar variant (Spec: array_dimensions empty
    /// → value with length 1).
    #[must_use]
    pub fn scalar(v: VariantValue) -> Self {
        Self {
            array_dimensions: Vec::new(),
            value: alloc::vec![v],
        }
    }

    /// `true` if the variant is a scalar (array_dimensions empty).
    #[must_use]
    pub fn is_scalar(&self) -> bool {
        self.array_dimensions.is_empty()
    }

    /// `true` if a 1D array (array_dimensions length 1).
    #[must_use]
    pub fn is_1d_array(&self) -> bool {
        self.array_dimensions.len() == 1
    }

    /// Returns the `BuiltinTypeKind` of all values. Returns `None` if the
    /// variant is empty or contains mixed-typed values.
    #[must_use]
    pub fn type_kind(&self) -> Option<BuiltinTypeKind> {
        let first = self.value.first()?.kind();
        if self.value.iter().all(|v| v.kind() == first) {
            Some(first)
        } else {
            None
        }
    }
}

/// Spec §8.2.2 — `VariantValue` Union switch (BuiltinTypeKind).
///
/// We provide named variants for all 25 spec cases.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantValue {
    /// `BOOLEAN_TYPE` Case.
    Boolean(bool),
    /// `SBYTE_TYPE` Case.
    SByte(i8),
    /// `BYTE_TYPE` Case.
    Byte(u8),
    /// `INT16_TYPE` Case.
    Int16(i16),
    /// `UINT16_TYPE` Case.
    UInt16(u16),
    /// `INT32_TYPE` Case.
    Int32(i32),
    /// `UINT32_TYPE` Case.
    UInt32(u32),
    /// `INT64_TYPE` Case.
    Int64(i64),
    /// `UINT64_TYPE` Case.
    UInt64(u64),
    /// `FLOAT_TYPE` Case.
    Float(f32),
    /// `DOUBLE_TYPE` Case.
    Double(f64),
    /// `STRING_TYPE` Case.
    String(String),
    /// `DATETIME_TYPE` Case (int64 ticks).
    DateTime(i64),
    /// `GUID_TYPE` Case.
    Guid(Guid),
    /// `BYTESTRING_TYPE` Case.
    ByteString(ByteString),
    /// `XMLELEMENT_TYPE` Case.
    XmlElement(String),
    /// `NODEID_TYPE` Case.
    NodeId(NodeId),
    /// `STATUSCODE_TYPE` Case.
    StatusCode(StatusCode),
    /// `QUALIFIEDNAME_TYPE` Case.
    QualifiedName(QualifiedName),
    /// `LOCALIZEDTEXT_TYPE` Case.
    LocalizedText(LocalizedText),
    /// `EXTENSIONOBJECT_TYPE` Case.
    ExtensionObject(ExtensionObject),
}

impl VariantValue {
    /// Returns the corresponding `BuiltinTypeKind`.
    #[must_use]
    pub const fn kind(&self) -> BuiltinTypeKind {
        match self {
            Self::Boolean(_) => BuiltinTypeKind::Boolean,
            Self::SByte(_) => BuiltinTypeKind::SByte,
            Self::Byte(_) => BuiltinTypeKind::Byte,
            Self::Int16(_) => BuiltinTypeKind::Int16,
            Self::UInt16(_) => BuiltinTypeKind::UInt16,
            Self::Int32(_) => BuiltinTypeKind::Int32,
            Self::UInt32(_) => BuiltinTypeKind::UInt32,
            Self::Int64(_) => BuiltinTypeKind::Int64,
            Self::UInt64(_) => BuiltinTypeKind::UInt64,
            Self::Float(_) => BuiltinTypeKind::Float,
            Self::Double(_) => BuiltinTypeKind::Double,
            Self::String(_) => BuiltinTypeKind::String,
            Self::DateTime(_) => BuiltinTypeKind::DateTime,
            Self::Guid(_) => BuiltinTypeKind::Guid,
            Self::ByteString(_) => BuiltinTypeKind::ByteString,
            Self::XmlElement(_) => BuiltinTypeKind::XmlElement,
            Self::NodeId(_) => BuiltinTypeKind::NodeId,
            Self::StatusCode(_) => BuiltinTypeKind::StatusCode,
            Self::QualifiedName(_) => BuiltinTypeKind::QualifiedName,
            Self::LocalizedText(_) => BuiltinTypeKind::LocalizedText,
            Self::ExtensionObject(_) => BuiltinTypeKind::ExtensionObject,
        }
    }
}

/// Spec §8.2.2 — `DataValue` with value + status + timestamps.
#[derive(Debug, Clone, PartialEq)]
pub struct DataValue {
    /// `@id(1) @optional Variant value`.
    pub value: Option<Variant>,
    /// `@id(2) @optional StatusCode status`.
    pub status: Option<StatusCode>,
    /// `@id(4) @optional DateTime source_timestamp` (int64 ticks).
    pub source_timestamp: Option<i64>,
    /// `@id(8) @optional DateTime server_timestamp` (int64 ticks).
    pub server_timestamp: Option<i64>,
    /// `@id(10) @optional uint16 source_pico_sec`.
    pub source_pico_sec: Option<u16>,
    /// `@id(32) @optional uint16 server_pico_sec`.
    pub server_pico_sec: Option<u16>,
}

impl DataValue {
    /// Constructor with value + status + source timestamp; server
    /// timestamps + picos set to `None`.
    #[must_use]
    pub const fn new_value(value: Variant, status: StatusCode, source_timestamp: i64) -> Self {
        Self {
            value: Some(value),
            status: Some(status),
            source_timestamp: Some(source_timestamp),
            server_timestamp: None,
            source_pico_sec: None,
            server_pico_sec: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_scalar_constructor_yields_empty_array_dimensions() {
        // Spec §8.2.2 — empty array_dimensions = scalar.
        let v = Variant::scalar(VariantValue::Int32(42));
        assert!(v.is_scalar());
        assert!(!v.is_1d_array());
        assert_eq!(v.value.len(), 1);
    }

    #[test]
    fn variant_1d_array_has_array_dimensions_length_1() {
        let v = Variant {
            array_dimensions: alloc::vec![3],
            value: alloc::vec![
                VariantValue::Int32(1),
                VariantValue::Int32(2),
                VariantValue::Int32(3),
            ],
        };
        assert!(!v.is_scalar());
        assert!(v.is_1d_array());
    }

    #[test]
    fn variant_value_kind_round_trips_for_all_cases() {
        // Spec — VariantValue cases are 1:1 with BuiltinTypeKind.
        let cases: alloc::vec::Vec<(VariantValue, BuiltinTypeKind)> = alloc::vec![
            (VariantValue::Boolean(true), BuiltinTypeKind::Boolean),
            (VariantValue::SByte(-1), BuiltinTypeKind::SByte),
            (VariantValue::Byte(1), BuiltinTypeKind::Byte),
            (VariantValue::Int16(-1), BuiltinTypeKind::Int16),
            (VariantValue::UInt16(1), BuiltinTypeKind::UInt16),
            (VariantValue::Int32(-1), BuiltinTypeKind::Int32),
            (VariantValue::UInt32(1), BuiltinTypeKind::UInt32),
            (VariantValue::Int64(-1), BuiltinTypeKind::Int64),
            (VariantValue::UInt64(1), BuiltinTypeKind::UInt64),
            (VariantValue::Float(1.0), BuiltinTypeKind::Float),
            (VariantValue::Double(1.0), BuiltinTypeKind::Double),
            (
                VariantValue::String(String::from("x")),
                BuiltinTypeKind::String
            ),
            (VariantValue::DateTime(0), BuiltinTypeKind::DateTime),
            (
                VariantValue::Guid(Guid::from_bytes([0; 16])),
                BuiltinTypeKind::Guid
            ),
            (
                VariantValue::ByteString(alloc::vec![]),
                BuiltinTypeKind::ByteString
            ),
            (
                VariantValue::XmlElement(String::from("<a/>")),
                BuiltinTypeKind::XmlElement
            ),
            (VariantValue::StatusCode(0), BuiltinTypeKind::StatusCode),
        ];
        for (v, k) in cases {
            assert_eq!(v.kind(), k);
        }
    }

    #[test]
    fn variant_type_kind_detects_homogeneous_arrays() {
        let v = Variant {
            array_dimensions: alloc::vec![2],
            value: alloc::vec![VariantValue::Int32(1), VariantValue::Int32(2)],
        };
        assert_eq!(v.type_kind(), Some(BuiltinTypeKind::Int32));
    }

    #[test]
    fn variant_type_kind_returns_none_for_mixed_arrays() {
        let v = Variant {
            array_dimensions: alloc::vec![2],
            value: alloc::vec![
                VariantValue::Int32(1),
                VariantValue::String(String::from("x"))
            ],
        };
        assert!(v.type_kind().is_none());
    }

    #[test]
    fn variant_type_kind_returns_none_for_empty_value() {
        let v = Variant {
            array_dimensions: alloc::vec![],
            value: alloc::vec![],
        };
        assert!(v.type_kind().is_none());
    }

    #[test]
    fn data_value_new_value_helper_sets_main_fields() {
        // Spec §8.2.2 DataValue.
        let v = Variant::scalar(VariantValue::Double(2.5));
        let dv = DataValue::new_value(v.clone(), 0, 1234);
        assert_eq!(dv.value, Some(v));
        assert_eq!(dv.status, Some(0));
        assert_eq!(dv.source_timestamp, Some(1234));
        assert!(dv.server_timestamp.is_none());
    }

    #[test]
    fn body_encoding_distinct_values() {
        // Spec @value(0)/(1)/(2).
        assert_ne!(BodyEncoding::None, BodyEncoding::ByteString);
        assert_ne!(BodyEncoding::ByteString, BodyEncoding::XmlElement);
    }
}
