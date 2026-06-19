// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Variant → DDS-Type Mapping — Spec Tab 8.16 + §8.4.3.3.
//!
//! Returns, per `(BuiltinTypeKind, ArrayShape)`, the DDS-IDL type
//! representation that the gateway must write into a `DdsOutput` field.
//! The three dimension cases from §8.4.3.3:
//!
//! 1. **Scalar** (`array_dimensions` empty) → DDS primitive type
//!    (`int32`, `boolean`, ...) or the built-in type aliases
//!    (`NodeId`, `LocalizedText`, ...).
//! 2. **1D-Array** (`array_dimensions.len() == 1`) → `sequence<T>`.
//! 3. **Multi-dim matrix** (`array_dimensions.len() > 1`) → a wrapper
//!    struct with `array: sequence<T>` + `array_dimensions:
//!    sequence<uint32>` (e.g. `Int32Matrix`).
//!
//! Spec Tab 8.16 lists the mapping per BuiltinType in each
//! dimension. We return all three forms per type kind as an IDL
//! type string — codegen consumers emit wire bindings from it.

use alloc::format;
use alloc::string::{String, ToString};

use crate::data_value::Variant;
use crate::types::BuiltinTypeKind;

/// Form-Kategorie aus §8.4.3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayShape {
    /// Scalar (`array_dimensions` empty).
    Scalar,
    /// 1D-Array (`array_dimensions.len() == 1`).
    Array1D,
    /// Multi-Dim-Matrix (`array_dimensions.len() > 1`).
    Matrix,
}

impl ArrayShape {
    /// Classifies a variant per §8.4.3.3.
    #[must_use]
    pub fn classify(v: &Variant) -> Self {
        match v.array_dimensions.len() {
            0 => Self::Scalar,
            1 => Self::Array1D,
            _ => Self::Matrix,
        }
    }
}

/// IDL type reference that the gateway writes into the DDS output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsTypeRef {
    /// IDL type spec (e.g. `int32`, `sequence<int32>`, `Int32Matrix`).
    pub idl_type: String,
}

impl DdsTypeRef {
    fn from_str(s: &str) -> Self {
        Self {
            idl_type: s.to_string(),
        }
    }
}

/// Spec Tab 8.16 Variant→DDS mapping — returns the IDL type for
/// `(builtin, shape)`.
#[must_use]
pub fn map_variant_to_dds(builtin: BuiltinTypeKind, shape: ArrayShape) -> DdsTypeRef {
    let scalar = scalar_idl(builtin);
    match shape {
        ArrayShape::Scalar => DdsTypeRef::from_str(scalar),
        ArrayShape::Array1D => {
            // Spec: 1D arrays get the alias `<Type>Array` as a
            // shortcut for `sequence<T>`. We provide both — the
            // alias form is explicitly listed in Spec Tab 8.16.
            DdsTypeRef {
                idl_type: format!("sequence<{scalar}>"),
            }
        }
        ArrayShape::Matrix => {
            // Spec: a multi-dim matrix is a wrapper struct
            // `<Type>Matrix { <Type>Array array; sequence<uint32>
            // array_dimensions; }`. We return the type name that
            // is to be generated as an IDL struct.
            DdsTypeRef::from_str(matrix_name(builtin))
        }
    }
}

fn scalar_idl(b: BuiltinTypeKind) -> &'static str {
    match b {
        BuiltinTypeKind::Boolean => "boolean",
        BuiltinTypeKind::SByte => "int8",
        BuiltinTypeKind::Byte => "uint8",
        BuiltinTypeKind::Int16 => "int16",
        BuiltinTypeKind::UInt16 => "uint16",
        BuiltinTypeKind::Int32 => "int32",
        BuiltinTypeKind::UInt32 => "uint32",
        BuiltinTypeKind::Int64 => "int64",
        BuiltinTypeKind::UInt64 => "uint64",
        BuiltinTypeKind::Float => "float",
        BuiltinTypeKind::Double => "double",
        BuiltinTypeKind::String => "string",
        BuiltinTypeKind::DateTime => "DateTime",
        BuiltinTypeKind::Guid => "Guid",
        BuiltinTypeKind::ByteString => "ByteString",
        BuiltinTypeKind::XmlElement => "XmlElement",
        BuiltinTypeKind::NodeId => "NodeId",
        BuiltinTypeKind::ExpandedNodeId => "ExpandedNodeId",
        BuiltinTypeKind::StatusCode => "StatusCode",
        BuiltinTypeKind::QualifiedName => "QualifiedName",
        BuiltinTypeKind::LocalizedText => "LocalizedText",
        BuiltinTypeKind::ExtensionObject => "ExtensionObject",
        BuiltinTypeKind::DataValue => "DataValue",
        BuiltinTypeKind::Variant => "BaseDataType",
        BuiltinTypeKind::DiagnosticInfo => "DiagnosticInfo",
    }
}

fn matrix_name(b: BuiltinTypeKind) -> &'static str {
    // Spec Tab 8.16: Matrix-Wrapper-Namen.
    match b {
        BuiltinTypeKind::Boolean => "BooleanMatrix",
        BuiltinTypeKind::SByte => "SByteMatrix",
        BuiltinTypeKind::Byte => "ByteMatrix",
        BuiltinTypeKind::Int16 => "Int16Matrix",
        BuiltinTypeKind::UInt16 => "UInt16Matrix",
        BuiltinTypeKind::Int32 => "Int32Matrix",
        BuiltinTypeKind::UInt32 => "UInt32Matrix",
        BuiltinTypeKind::Int64 => "Int64Matrix",
        BuiltinTypeKind::UInt64 => "UInt64Matrix",
        BuiltinTypeKind::Float => "FloatMatrix",
        BuiltinTypeKind::Double => "DoubleMatrix",
        BuiltinTypeKind::String => "StringMatrix",
        BuiltinTypeKind::DateTime => "DateTimeMatrix",
        BuiltinTypeKind::Guid => "GuidMatrix",
        BuiltinTypeKind::ByteString => "ByteStringMatrix",
        BuiltinTypeKind::XmlElement => "XmlElementMatrix",
        BuiltinTypeKind::NodeId => "NodeIdMatrix",
        BuiltinTypeKind::ExpandedNodeId => "ExpandedNodeIdMatrix",
        BuiltinTypeKind::StatusCode => "StatusCodeMatrix",
        BuiltinTypeKind::QualifiedName => "QualifiedNameMatrix",
        BuiltinTypeKind::LocalizedText => "LocalizedTextMatrix",
        BuiltinTypeKind::ExtensionObject => "ExtensionObjectMatrix",
        BuiltinTypeKind::DataValue => "DataValueMatrix",
        BuiltinTypeKind::Variant => "BaseDataTypeMatrix",
        BuiltinTypeKind::DiagnosticInfo => "DiagnosticInfoMatrix",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::data_value::VariantValue;

    #[test]
    fn classify_scalar_array_matrix() {
        let s = Variant::scalar(VariantValue::Int32(7));
        assert_eq!(ArrayShape::classify(&s), ArrayShape::Scalar);

        let a = Variant {
            array_dimensions: alloc::vec![3],
            value: alloc::vec![
                VariantValue::Int32(1),
                VariantValue::Int32(2),
                VariantValue::Int32(3),
            ],
        };
        assert_eq!(ArrayShape::classify(&a), ArrayShape::Array1D);

        let m = Variant {
            array_dimensions: alloc::vec![2, 3],
            value: alloc::vec![],
        };
        assert_eq!(ArrayShape::classify(&m), ArrayShape::Matrix);
    }

    #[test]
    fn primitive_scalar_mappings_match_spec_tab_816() {
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Boolean, ArrayShape::Scalar).idl_type,
            "boolean"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Int32, ArrayShape::Scalar).idl_type,
            "int32"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::SByte, ArrayShape::Scalar).idl_type,
            "int8"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Byte, ArrayShape::Scalar).idl_type,
            "uint8"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Double, ArrayShape::Scalar).idl_type,
            "double"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::String, ArrayShape::Scalar).idl_type,
            "string"
        );
    }

    #[test]
    fn builtin_scalar_mappings_match_spec() {
        // Spec Tab 8.16 — the non-primitive built-in types are
        // mapped to their spec aliases (no decay to primitives).
        for (b, expected) in [
            (BuiltinTypeKind::DateTime, "DateTime"),
            (BuiltinTypeKind::Guid, "Guid"),
            (BuiltinTypeKind::ByteString, "ByteString"),
            (BuiltinTypeKind::NodeId, "NodeId"),
            (BuiltinTypeKind::ExpandedNodeId, "ExpandedNodeId"),
            (BuiltinTypeKind::StatusCode, "StatusCode"),
            (BuiltinTypeKind::QualifiedName, "QualifiedName"),
            (BuiltinTypeKind::LocalizedText, "LocalizedText"),
            (BuiltinTypeKind::ExtensionObject, "ExtensionObject"),
            (BuiltinTypeKind::DataValue, "DataValue"),
            (BuiltinTypeKind::Variant, "BaseDataType"),
            (BuiltinTypeKind::DiagnosticInfo, "DiagnosticInfo"),
        ] {
            assert_eq!(
                map_variant_to_dds(b, ArrayShape::Scalar).idl_type,
                expected,
                "scalar mapping for {b:?}"
            );
        }
    }

    #[test]
    fn array_1d_wraps_in_sequence() {
        // Spec Tab 8.16 — 1D arrays = sequence<T> (aliases
        // `Int32Array` etc. are equivalent).
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Int32, ArrayShape::Array1D).idl_type,
            "sequence<int32>"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::String, ArrayShape::Array1D).idl_type,
            "sequence<string>"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::NodeId, ArrayShape::Array1D).idl_type,
            "sequence<NodeId>"
        );
    }

    #[test]
    fn matrix_returns_named_struct() {
        // Spec Tab 8.16 — Multi-Dim ist `<Type>Matrix`-Struct.
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Int32, ArrayShape::Matrix).idl_type,
            "Int32Matrix"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Boolean, ArrayShape::Matrix).idl_type,
            "BooleanMatrix"
        );
        assert_eq!(
            map_variant_to_dds(BuiltinTypeKind::Float, ArrayShape::Matrix).idl_type,
            "FloatMatrix"
        );
    }

    #[test]
    fn all_25_builtins_have_matrix_alias() {
        // Sanity: each of the 25 built-in type kinds returns a
        // non-empty IDL type string in every form.
        let all = [
            BuiltinTypeKind::Boolean,
            BuiltinTypeKind::SByte,
            BuiltinTypeKind::Byte,
            BuiltinTypeKind::Int16,
            BuiltinTypeKind::UInt16,
            BuiltinTypeKind::Int32,
            BuiltinTypeKind::UInt32,
            BuiltinTypeKind::Int64,
            BuiltinTypeKind::UInt64,
            BuiltinTypeKind::Float,
            BuiltinTypeKind::Double,
            BuiltinTypeKind::String,
            BuiltinTypeKind::DateTime,
            BuiltinTypeKind::Guid,
            BuiltinTypeKind::ByteString,
            BuiltinTypeKind::XmlElement,
            BuiltinTypeKind::NodeId,
            BuiltinTypeKind::ExpandedNodeId,
            BuiltinTypeKind::StatusCode,
            BuiltinTypeKind::QualifiedName,
            BuiltinTypeKind::LocalizedText,
            BuiltinTypeKind::ExtensionObject,
            BuiltinTypeKind::DataValue,
            BuiltinTypeKind::Variant,
            BuiltinTypeKind::DiagnosticInfo,
        ];
        assert_eq!(all.len(), 25);
        for b in all {
            for shape in [ArrayShape::Scalar, ArrayShape::Array1D, ArrayShape::Matrix] {
                let r = map_variant_to_dds(b, shape);
                assert!(!r.idl_type.is_empty(), "{b:?} {shape:?}");
            }
        }
    }
}
